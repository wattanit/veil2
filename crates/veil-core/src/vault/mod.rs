//! Public API: create, open, ingest, extract, replace, delete, verify
//! (Spec §2, §5.1). Nothing here needs a terminal or a prompt (A-1).
//!
//! Not built yet: compaction and orphaned-pack cleanup (Phase 4), NFC name
//! normalisation (Phase 5) — so name comparison here is exact on stored bytes.

mod limits;
mod lock;
mod progress;
mod verify;
mod walk;

pub use limits::{Limits, MAX_ENTRIES_PER_VAULT, MAX_FILE_SIZE};
pub use lock::{Access, LOCK_FILE, VaultLock};
pub use progress::{Cancel, NoProgress, Progress, ProgressReport, Unit};
pub use verify::{Outcome, Report, Verdict};
pub use walk::{Found, SkipReason, Skipped, Walk, walk};

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::crypto::{
    CryptoError, Dek, EntryWrapKey, IndexKey, KdfAlgorithm, KdfParams, MasterKey, NONCE_PREFIX_LEN,
    Password, WRAP_NONCE_LEN, decrypt_watched, derive_kek, encrypt_watched, entry_wrap_key,
    generate_dek, generate_master_key, generate_nonce_prefix, index_key, unwrap_dek, wrap_dek,
    wrap_master_key,
};
use crate::error::{Damaged, Error, Limit, Result};
use crate::format::{CURRENT_FORMAT_VERSION, Header, SALT_LEN, unlock};
use crate::index::{Entry, EntryId, IndexDocument, Statistics};
use crate::store::{DEFAULT_PACK_CAP, PackSink, PackSource, total_pack_bytes};

/// Name of the header file within a vault directory (Spec §4.1).
pub const HEADER_FILE: &str = "veil.header";

/// What a folder ingest did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FolderOutcome {
    /// Every entry added, in the order stored.
    pub added: Vec<EntryId>,
    /// Every path the walk declined, with its reason (FR-11).
    pub skipped: Vec<Skipped>,
}

/// An open vault. Holds no process-global state, so several can be open at
/// once (A-7).
pub struct Vault {
    dir: PathBuf,
    header: Header,
    // The master key is deliberately not kept. It is consumed at open to derive
    // these two subkeys and then zeroised; a password change re-unwraps it from
    // the header on disk, which it has to read anyway.
    index_key: IndexKey,
    entry_wrap_key: EntryWrapKey,
    document: IndexDocument,
    pack_cap: u64,
    limits: Limits,
    lock: VaultLock,
}

impl Vault {
    /// Creates a vault at `dir`. `pack_cap` is a parameter so multi-pack
    /// behaviour is testable without gigabytes of fixture.
    ///
    /// # Errors
    ///
    /// [`Error::Io`], [`Error::VaultInUse`], or a cryptographic failure.
    pub fn create(
        dir: &Path,
        password: &Password,
        params: KdfParams,
        pack_cap: u64,
    ) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let lock = VaultLock::acquire(dir)?;

        let mut kdf_salt = [0u8; SALT_LEN];
        let mut wrap_nonce = [0u8; WRAP_NONCE_LEN];
        fill_random(&mut kdf_salt)?;
        fill_random(&mut wrap_nonce)?;

        let master = generate_master_key();
        let kek = derive_kek(KdfAlgorithm::Argon2id, params, &kdf_salt, password)?;

        let mut header = Header {
            format_version: CURRENT_FORMAT_VERSION,
            writer_version: writer_version(),
            kdf_algorithm: KdfAlgorithm::Argon2id,
            kdf_params: params,
            kdf_salt,
            wrap_nonce,
            wrapped_master_key: [0u8; 48],
        };
        let staged = header.to_bytes();
        header.wrapped_master_key =
            wrap_master_key(&kek, &wrap_nonce, Header::prefix(&staged), &master)?;

        write_header(dir, &header)?;

        let vault = Self::assemble(
            dir.to_path_buf(),
            header,
            master,
            IndexDocument::empty(),
            pack_cap,
            lock,
        );
        crate::index::write(&vault.dir, &vault.index_key, &vault.document)?;
        Ok(vault)
    }

    /// Opens a vault, decrypting the whole index into memory (FR-6).
    ///
    /// Touches no pack file, so open cost follows entry count and not vault
    /// size (S-2). Never verifies content — that reads everything (FR-33).
    ///
    /// # Errors
    ///
    /// [`Error::NotAVault`], [`Error::FormatTooNew`], [`Error::WrongPassword`],
    /// [`Error::VaultInUse`], or [`Error::Corrupt`].
    pub fn open(dir: &Path, password: &Password) -> Result<Self> {
        let bytes = std::fs::read(dir.join(HEADER_FILE)).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::NotAVault
            } else {
                Error::from(e)
            }
        })?;

        let lock = VaultLock::acquire(dir)?;
        let (header, master) = unlock(&bytes, password)?;
        let key = index_key(&master);
        let document = crate::index::read(dir, &key)?;

        Ok(Self::assemble(
            dir.to_path_buf(),
            header,
            master,
            document,
            DEFAULT_PACK_CAP,
            lock,
        ))
    }

    /// Closes the vault, releasing its lock and zeroising its keys (FR-3).
    ///
    /// Takes `self` rather than setting a flag, so a closed vault is not
    /// reachable as a value.
    pub fn lock(self) {
        drop(self);
    }

    fn assemble(
        dir: PathBuf,
        header: Header,
        master: MasterKey,
        mut document: IndexDocument,
        pack_cap: u64,
        lock: VaultLock,
    ) -> Self {
        // Repaired upward, never downward: identifiers are bound into nonces,
        // so a counter that went backwards would reissue one.
        let highest = document.entries.iter().map(|e| e.id.get()).max();
        let floor = highest.map_or(1, |h| h + 1);
        document.next_entry_id = document.next_entry_id.max(floor).max(1);

        Self {
            index_key: index_key(&master),
            entry_wrap_key: entry_wrap_key(&master),
            dir,
            header,
            document,
            pack_cap,
            limits: Limits::default(),
            lock,
        }
    }

    /// Sets the limits this vault enforces (FR-15, C-1, C-2).
    pub fn set_limits(&mut self, limits: Limits) {
        self.limits = limits;
    }

    /// The limits currently enforced.
    #[must_use]
    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// Whether this vault may be written (§4.5, §4.8).
    #[must_use]
    pub fn access(&self) -> Access {
        self.lock.access()
    }

    /// The complete index, served from memory (FR-6).
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.document.entries
    }

    /// The entry at one full path — folder and name together (FR-13, §4.6).
    ///
    /// Identity is the full path: matching on name alone would let an ingest
    /// into one folder overwrite a file in another.
    #[must_use]
    pub fn find(&self, folder: &str, name: &str) -> Option<&Entry> {
        self.document
            .entries
            .iter()
            .find(|e| e.folder == folder && e.name == name)
    }

    /// The vault's totals, read rather than computed (FR-8, FR-22).
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        self.document.statistics
    }

    /// The index generation — the external-modification detector (FR-27).
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.document.generation
    }

    /// The parsed header, for provenance and diagnostics.
    #[must_use]
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Recomputes the statistics by scanning. A test oracle and a diagnostic,
    /// never the source of [`statistics`](Self::statistics) — FR-22 requires
    /// those to be incremental. This is what they get checked against.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the packs cannot be measured.
    pub fn recount_statistics(&self) -> Result<Statistics> {
        let physical = total_pack_bytes(&self.dir)?;
        let referenced: u64 = self
            .document
            .entries
            .iter()
            .flat_map(|e| e.extents.iter())
            .map(|x| x.length)
            .sum();
        Ok(Statistics {
            entry_count: self.document.entries.len() as u64,
            logical_bytes: self.document.entries.iter().map(|e| e.size).sum(),
            physical_bytes: physical,
            reclaimable_bytes: physical.saturating_sub(referenced),
        })
    }

    // -- writes ------------------------------------------------------------

    /// The one place every write passes through, so FR-27's check lives here:
    /// the generation counter only detects anything if something reads it
    /// before writing.
    fn begin_write(&self) -> Result<()> {
        if self.lock.access() == Access::ReadOnly {
            return Err(Error::ReadOnly);
        }
        let on_disk = crate::index::generations(&self.dir)
            .into_iter()
            .flatten()
            .max()
            .unwrap_or(0);
        if on_disk > self.document.generation {
            return Err(Error::ChangedOnDisk);
        }
        Ok(())
    }

    /// Streams one source into the packs, producing an entry nothing yet
    /// references. Advances no generation, so a failure or cancellation rolls
    /// the packs back and the index never learned of the attempt.
    #[allow(clippy::too_many_arguments)]
    fn stage(
        &self,
        id: EntryId,
        name: &str,
        folder: &str,
        src: &mut impl Read,
        total: Option<u64>,
        progress: &mut impl Progress,
        cancel: &Cancel,
    ) -> Result<(Entry, u64)> {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled { rolled_back: true });
        }

        let dek = generate_dek();
        let nonce_prefix = generate_nonce_prefix();
        let max_file_size = self.limits.max_file_size;

        let mut sink = PackSink::open(&self.dir, self.pack_cap)?;
        let mut stop: Option<Error> = None;

        let outcome = {
            // The size limit is checked here rather than from the source's
            // stated length: metadata is a limit on files, not on content, and
            // every non-file source would slip past it (FR-15, C-2).
            let mut hook = |done: u64| -> std::result::Result<(), CryptoError> {
                if done > max_file_size {
                    stop = Some(Error::LimitExceeded {
                        limit: Limit::FileSize,
                        allowed: max_file_size,
                        actual: done,
                    });
                    return Err(CryptoError::Stopped);
                }
                if cancel.is_cancelled() {
                    stop = Some(Error::Cancelled { rolled_back: true });
                    return Err(CryptoError::Stopped);
                }
                progress.report(ProgressReport {
                    unit: Unit::Bytes,
                    done,
                    total,
                });
                Ok(())
            };
            encrypt_watched(&dek, &nonce_prefix, id.get(), src, &mut sink, &mut hook)
        };

        let summary = match outcome {
            Ok(summary) => summary,
            Err(e) => {
                sink.rollback()?;
                return Err(stop.unwrap_or_else(|| Error::from(e)));
            }
        };

        // Pack data is durable before anything may refer to it (FR-12).
        let extents = sink.finish()?;

        let entry = Entry {
            id,
            name: name.to_owned(),
            folder: folder.to_owned(),
            size: summary.plaintext_len,
            source_mtime: now(),
            added_at: now(),
            content_hash: summary.hash,
            wrapped_dek: wrap_dek(&self.entry_wrap_key, id.get(), &dek)?,
            nonce_prefix,
            extents,
            unknown: std::collections::BTreeMap::new(),
        };
        Ok((entry, summary.ciphertext_len))
    }

    /// Stores one source's content under the given name and folder.
    ///
    /// Content is written and fsynced before the index generation that names it
    /// advances, and success is reported only after the index write returns
    /// (FR-12). A crash between the two leaves unreferenced pack bytes, never
    /// an index entry pointing at bytes that were not durable.
    ///
    /// # Errors
    ///
    /// [`Error::LimitExceeded`], [`Error::Cancelled`], [`Error::ChangedOnDisk`],
    /// or [`Error::Io`].
    pub fn add(
        &mut self,
        name: &str,
        folder: &str,
        src: &mut impl Read,
        progress: &mut impl Progress,
        cancel: &Cancel,
    ) -> Result<EntryId> {
        self.begin_write()?;

        let count = self.document.entries.len() as u64;
        if count >= self.limits.max_entries {
            return Err(Error::LimitExceeded {
                limit: Limit::EntriesPerVault,
                allowed: self.limits.max_entries,
                actual: count + 1,
            });
        }

        let id = EntryId::new(self.document.next_entry_id);
        let (entry, ciphertext_len) = self.stage(id, name, folder, src, None, progress, cancel)?;

        self.document.next_entry_id += 1;
        self.document.statistics.entry_count += 1;
        self.document.statistics.logical_bytes += entry.size;
        self.document.statistics.physical_bytes += ciphertext_len;
        self.document.entries.push(entry);
        self.commit()?;
        Ok(id)
    }

    /// Stores one file from the filesystem. The source is opened read-only and
    /// left as it was; nothing here deletes or modifies a file outside a vault
    /// (FR-9).
    ///
    /// # Errors
    ///
    /// As [`add`](Self::add), plus [`Error::Io`] if the source cannot be read.
    pub fn add_path(
        &mut self,
        src_path: &Path,
        folder: &str,
        progress: &mut impl Progress,
        cancel: &Cancel,
    ) -> Result<EntryId> {
        let name = src_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or(Error::Io {
                kind: std::io::ErrorKind::InvalidInput,
            })?
            .to_owned();
        let mut file = std::fs::File::open(src_path)?;
        self.add(&name, folder, &mut file, progress, cancel)
    }

    /// Stores every regular file beneath `root` (FR-10, FR-11).
    ///
    /// Each file records its path relative to `root` as folder metadata.
    /// Symbolic links are not followed and are returned as skipped, so a caller
    /// can say what was left out (FR-11).
    ///
    /// Progress counts entries, not bytes — forwarding each file's byte counter
    /// would reset the figure to zero at every file. For byte-level progress on
    /// one file, drive [`add_path`](Self::add_path).
    ///
    /// # Errors
    ///
    /// As [`add`](Self::add). Each file is its own transaction, so a failure
    /// partway leaves the files already stored in place.
    pub fn add_folder(
        &mut self,
        root: &Path,
        progress: &mut impl Progress,
        cancel: &Cancel,
    ) -> Result<FolderOutcome> {
        let found = walk(root)?;
        let total = Some(found.files.len() as u64);
        let mut added = Vec::with_capacity(found.files.len());

        for file in &found.files {
            let mut source = std::fs::File::open(&file.path)?;
            added.push(self.add(
                &file.name,
                &file.folder,
                &mut source,
                &mut NoProgress,
                cancel,
            )?);
            progress.report(ProgressReport {
                unit: Unit::Entries,
                done: added.len() as u64,
                total,
            });
        }

        Ok(FolderOutcome {
            added,
            skipped: found.skipped,
        })
    }

    /// Replaces the entry at one full path with new content (FR-13).
    ///
    /// New content is written and made durable first, then **one** generation
    /// step both points the path at it and marks the old extents reclaimable.
    /// Remove-then-add would leave a window with zero intact versions (HC-4).
    ///
    /// # Errors
    ///
    /// [`Error::Corrupt`] with an empty affected list if no entry has that
    /// path; otherwise as [`add`](Self::add).
    pub fn replace(
        &mut self,
        folder: &str,
        name: &str,
        src: &mut impl Read,
        progress: &mut impl Progress,
        cancel: &Cancel,
    ) -> Result<EntryId> {
        self.begin_write()?;

        let Some(position) = self
            .document
            .entries
            .iter()
            .position(|e| e.folder == folder && e.name == name)
        else {
            return Err(Error::Corrupt {
                what: Damaged::Content,
                affected: Vec::new(),
            });
        };

        let id = EntryId::new(self.document.next_entry_id);
        let (entry, ciphertext_len) = self.stage(id, name, folder, src, None, progress, cancel)?;

        // Nothing below can fail before the single index write. That is what
        // keeps this one generation step rather than two.
        let old = self.document.entries.swap_remove(position);
        let old_stored: u64 = old.extents.iter().map(|x| x.length).sum();

        self.document.next_entry_id += 1;
        self.document.statistics.logical_bytes =
            self.document.statistics.logical_bytes - old.size + entry.size;
        self.document.statistics.physical_bytes += ciphertext_len;
        self.document.statistics.reclaimable_bytes += old_stored;
        self.document.entries.push(entry);
        self.commit()?;
        Ok(id)
    }

    /// Removes an entry from the index (FR-21).
    ///
    /// The stored bytes stay until compaction and are counted into reclaimable
    /// bytes so the reported figures say so (FR-8, FR-29). They cannot be
    /// removed here: packs are shared and append-only, so removing them means
    /// rewriting the pack, which is compaction (FR-23).
    ///
    /// # Errors
    ///
    /// [`Error::Corrupt`] with an empty affected list if no such entry exists;
    /// [`Error::ChangedOnDisk`] or [`Error::Io`].
    pub fn delete(&mut self, id: EntryId) -> Result<()> {
        self.begin_write()?;

        let Some(position) = self.document.entries.iter().position(|e| e.id == id) else {
            return Err(Error::Corrupt {
                what: Damaged::Content,
                affected: Vec::new(),
            });
        };

        let removed = self.document.entries.swap_remove(position);
        let stored: u64 = removed.extents.iter().map(|x| x.length).sum();

        self.document.statistics.entry_count -= 1;
        self.document.statistics.logical_bytes -= removed.size;
        self.document.statistics.reclaimable_bytes += stored;
        // `next_entry_id` is deliberately untouched: reissuing a deleted
        // entry's identifier would let its wrapped key decrypt under a live
        // entry's nonce.
        self.commit()
    }

    /// Changes the vault's password (FR-4).
    ///
    /// Only the master key's wrapping changes — no content, no index, no entry
    /// key — so the time it takes does not depend on vault size. The old
    /// password is verified before anything is written.
    ///
    /// # Errors
    ///
    /// [`Error::WrongPassword`] if `old` does not open the vault; otherwise
    /// [`Error::Io`].
    pub fn change_password(
        &mut self,
        old: &Password,
        new: &Password,
        params: KdfParams,
    ) -> Result<()> {
        if self.lock.access() == Access::ReadOnly {
            return Err(Error::ReadOnly);
        }

        // Verified against what is on disk, not what is in memory: the question
        // is whether the caller knows the password that opens this vault now.
        let bytes = std::fs::read(self.dir.join(HEADER_FILE))?;
        let (_, master) = unlock(&bytes, old)?;

        let mut kdf_salt = [0u8; SALT_LEN];
        let mut wrap_nonce = [0u8; WRAP_NONCE_LEN];
        // Fresh salt and nonce. A reused nonce under a rederivable key is a
        // break, not a weakness.
        fill_random(&mut kdf_salt)?;
        fill_random(&mut wrap_nonce)?;

        let kek = derive_kek(KdfAlgorithm::Argon2id, params, &kdf_salt, new)?;

        let mut header = self.header;
        header.writer_version = writer_version();
        header.kdf_algorithm = KdfAlgorithm::Argon2id;
        header.kdf_params = params;
        header.kdf_salt = kdf_salt;
        header.wrap_nonce = wrap_nonce;
        header.wrapped_master_key = [0u8; 48];
        let staged = header.to_bytes();
        header.wrapped_master_key =
            wrap_master_key(&kek, &wrap_nonce, Header::prefix(&staged), &master)?;

        write_header(&self.dir, &header)?;
        self.header = header;
        Ok(())
    }

    /// Re-reads the index from disk, adopting an external writer's change
    /// (FR-27). The way forward after [`Error::ChangedOnDisk`], without asking
    /// for the password again.
    ///
    /// Entry identifiers held from before are stale afterwards; re-read
    /// [`entries`](Self::entries).
    ///
    /// # Errors
    ///
    /// [`Error::Corrupt`] if the index on disk cannot be read.
    pub fn reload(&mut self) -> Result<()> {
        let document = crate::index::read(&self.dir, &self.index_key)?;
        let highest = document.entries.iter().map(|e| e.id.get()).max();
        let floor = highest.map_or(1, |h| h + 1);
        self.document = document;
        self.document.next_entry_id = self.document.next_entry_id.max(floor).max(1);
        Ok(())
    }

    fn commit(&mut self) -> Result<()> {
        self.document.generation += 1;
        crate::index::write(&self.dir, &self.index_key, &self.document)
    }

    // -- reads -------------------------------------------------------------

    /// Writes one entry's content to `dst`, verified.
    ///
    /// Nothing reaches `dst` before it authenticates, and the content hash is
    /// compared after the final chunk (FR-17). Takes a `Write` rather than a
    /// path, so no destination is ever chosen in here.
    ///
    /// # Errors
    ///
    /// [`Error::Corrupt`] naming the entry when the content is damaged, or
    /// [`Error::Cancelled`].
    pub fn extract(
        &self,
        id: EntryId,
        dst: &mut impl Write,
        progress: &mut impl Progress,
        cancel: &Cancel,
    ) -> Result<()> {
        let Some(entry) = self.document.entries.iter().find(|e| e.id == id) else {
            return Err(Error::Corrupt {
                what: Damaged::Content,
                affected: vec![id],
            });
        };
        self.read_entry(entry, dst, progress, cancel)
    }

    /// Extracts one entry to a path, removing the partial output on failure
    /// (FR-17). Lives here rather than in each frontend so the removal is
    /// written once.
    ///
    /// # Errors
    ///
    /// As [`extract`](Self::extract), plus [`Error::Io`] if the destination
    /// cannot be created.
    pub fn extract_to_path(
        &self,
        id: EntryId,
        dst_path: &Path,
        progress: &mut impl Progress,
        cancel: &Cancel,
    ) -> Result<()> {
        let mut file = std::fs::File::create(dst_path)?;
        let outcome = self
            .extract(id, &mut file, progress, cancel)
            .and_then(|()| file.sync_all().map_err(Error::from));
        drop(file);

        if outcome.is_err() {
            // A truncated plaintext on disk is indistinguishable from a short
            // file. If the removal itself fails, the original error is still
            // the one the caller needs.
            let _ = std::fs::remove_file(dst_path);
        }
        outcome
    }

    fn read_entry(
        &self,
        entry: &Entry,
        dst: &mut impl Write,
        progress: &mut impl Progress,
        cancel: &Cancel,
    ) -> Result<()> {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled { rolled_back: true });
        }

        let id = entry.id;
        let dek: Dek = unwrap_dek(&self.entry_wrap_key, id.get(), &entry.wrapped_dek)?;
        let mut source = PackSource::new(&self.dir, &entry.extents);
        let prefix: [u8; NONCE_PREFIX_LEN] = entry.nonce_prefix;
        let total = Some(entry.size);
        let mut stop: Option<Error> = None;

        let outcome = {
            let mut hook = |done: u64| -> std::result::Result<(), CryptoError> {
                if cancel.is_cancelled() {
                    stop = Some(Error::Cancelled { rolled_back: true });
                    return Err(CryptoError::Stopped);
                }
                progress.report(ProgressReport {
                    unit: Unit::Bytes,
                    done,
                    total,
                });
                Ok(())
            };
            decrypt_watched(
                &dek,
                &prefix,
                id.get(),
                Some(&entry.content_hash),
                &mut source,
                dst,
                &mut hook,
            )
        };

        match outcome {
            Ok(_) => Ok(()),
            Err(CryptoError::Stopped) => {
                Err(stop.unwrap_or(Error::Cancelled { rolled_back: true }))
            }
            Err(CryptoError::ContentHashMismatch) => Err(Error::Corrupt {
                what: Damaged::ContentHash,
                affected: vec![id],
            }),
            // An I/O failure reaching the extents usually means a pack is
            // gone. Name the pack rather than folding it into "content is
            // damaged" (S-4).
            Err(CryptoError::Io) => {
                let missing = entry
                    .extents
                    .iter()
                    .find(|x| !crate::store::pack_path(&self.dir, x.pack_id).exists());
                Err(match missing {
                    Some(extent) => crate::store::damaged_pack(extent.pack_id, vec![id]),
                    None => Error::Corrupt {
                        what: Damaged::Content,
                        affected: vec![id],
                    },
                })
            }
            Err(_) => Err(Error::Corrupt {
                what: Damaged::Content,
                affected: vec![id],
            }),
        }
    }

    /// Verifies every entry, writing nothing (FR-33, §4.8).
    ///
    /// Reuses the extraction path with the output discarded, so "verification
    /// passed" means "extraction will succeed". Failure is per entry:
    /// verification continues and returns every failure, so one damaged pack
    /// yields a full list of what it cost (S-4).
    ///
    /// # Errors
    ///
    /// Never for a damaged entry — that is a verdict. Only if the report itself
    /// cannot be produced.
    pub fn verify(&self, progress: &mut impl Progress, cancel: &Cancel) -> Result<Report> {
        let total = Some(self.document.entries.len() as u64);
        let mut report = Report {
            verdicts: Vec::with_capacity(self.document.entries.len()),
            complete: true,
        };

        for (index, entry) in self.document.entries.iter().enumerate() {
            if cancel.is_cancelled() {
                report.complete = false;
                break;
            }

            // Per entry rather than per byte (§4.8).
            let outcome =
                match self.read_entry(entry, &mut std::io::sink(), &mut NoProgress, &Cancel::new())
                {
                    Ok(()) => Outcome::Passed,
                    Err(Error::Corrupt { what, .. }) => Outcome::Failed(what),
                    Err(_) => Outcome::Failed(Damaged::Content),
                };
            report.verdicts.push(Verdict {
                id: entry.id,
                outcome,
            });
            progress.report(ProgressReport {
                unit: Unit::Entries,
                done: index as u64 + 1,
                total,
            });
        }

        Ok(report)
    }
}

/// Writes the header, replacing any existing one.
///
/// Written beside and renamed over: a failure partway leaves the previous
/// header intact rather than a half-written one (HC-4).
fn write_header(dir: &Path, header: &Header) -> Result<()> {
    let final_path = dir.join(HEADER_FILE);
    let staging = dir.join(format!("{HEADER_FILE}.new"));

    {
        let mut file = std::fs::File::create(&staging)?;
        file.write_all(&header.to_bytes())?;
        file.sync_all()?;
    }
    std::fs::rename(&staging, &final_path)?;
    Ok(())
}

fn fill_random(buf: &mut [u8]) -> Result<()> {
    getrandom::fill(buf).map_err(|_| Error::Io {
        kind: std::io::ErrorKind::Other,
    })
}

fn writer_version() -> [u16; 3] {
    // Provenance only; never gates access (HC-5).
    let mut parts = env!("CARGO_PKG_VERSION").split('.');
    let mut next = || parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    [next(), next(), next()]
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
