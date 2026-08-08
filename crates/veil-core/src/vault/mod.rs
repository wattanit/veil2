//! Public API, orchestration, locking, progress, and cancellation
//! (Spec §2, §5.1).
//!
//! **This is the surface both frontends drive** (A-1, A-4). Nothing here needs
//! a terminal, a prompt, or a process boundary, which is the property that
//! separates this rebuild from an original whose logic could not be exercised
//! without a pseudo-terminal.
//!
//! Not here yet, and scheduled rather than forgotten: compaction (FR-23, FR-24)
//! and reconciliation of orphaned packs at open (FR-32) are Phase 4; NFC
//! normalisation of names (§4.6) is Phase 5, so comparison here is exact on the
//! stored form; crash injection at every fsync boundary (Spec §9) is Phase 4,
//! which is what proves the ordering established here survives interruption.

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

/// An open vault.
///
/// **An instance value, not a singleton** (A-7). Nothing here is process
/// global, so the single-vault limit stays a product decision rather than a
/// structural one, and supporting several open vaults later is a caller-side
/// change.
pub struct Vault {
    dir: PathBuf,
    header: Header,
    /// **The master key is not held.** It is consumed at open to derive the
    /// subkeys and then dropped and zeroised. Nothing an open vault does needs
    /// it: content keys come from the entry-wrap subkey, and a password change
    /// re-unwraps from the header on disk, which is what it must verify against
    /// anyway. Keeping it resident would extend the lifetime of the one key
    /// that opens everything, for no operation (HC-2, Spec §3.1).
    index_key: IndexKey,
    entry_wrap_key: EntryWrapKey,
    document: IndexDocument,
    pack_cap: u64,
    limits: Limits,
    /// Held for the vault's lifetime; released by `Drop` (FR-26).
    lock: VaultLock,
}

impl Vault {
    /// Creates a vault at `dir`.
    ///
    /// `pack_cap` is a parameter rather than a constant so that multi-pack
    /// behaviour — spanning, and the damage locality of S-4 — is testable
    /// without gigabytes of fixture.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the directory cannot be written, [`Error::VaultInUse`]
    /// if something already holds the directory open, or a cryptographic
    /// failure during setup.
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

    /// Opens a vault.
    ///
    /// **The whole index is decrypted here and held in memory** (FR-6), and no
    /// pack file is touched: the work at open is a function of entry count
    /// alone, which is S-2. Verification is never triggered at open (FR-33) —
    /// it reads the entire vault, and doing that on every open would make the
    /// product unusable at the sizes it exists for.
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

    /// Closes the vault, releasing its lock and destroying its keys (FR-3).
    ///
    /// **Consuming, not a flag.** A locked vault that remained reachable as a
    /// value would be a vault whose keys are one bug away from being used
    /// again; taking `self` makes the guarantee structural. Zeroisation happens
    /// in the key types' `Drop` (HC-2).
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
        // A document written before `next_entry_id` existed, or one whose
        // counter somehow lags its entries, is repaired upward and never
        // downward. Identifiers are bound into nonces (§3.2, §3.3); a counter
        // that went backwards would reissue one.
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

    /// Sets the pack cap for subsequent writes.
    ///
    /// Reads follow whatever extents an entry already records, so changing
    /// this never invalidates stored content.
    pub fn set_pack_cap(&mut self, cap: u64) {
        self.pack_cap = cap;
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
    /// **Identity is the full path.** Two entries sharing a name in different
    /// folders are unrelated, and matching on name alone would let an ingest
    /// into one folder silently overwrite a file in another.
    #[must_use]
    pub fn find(&self, folder: &str, name: &str) -> Option<&Entry> {
        self.document
            .entries
            .iter()
            .find(|e| e.folder == folder && e.name == name)
    }

    /// The vault's totals (FR-8), read rather than computed (FR-22).
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        self.document.statistics
    }

    /// The index generation, which is the external-modification detector
    /// (FR-27).
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.document.generation
    }

    /// The parsed header, for provenance and diagnostics.
    #[must_use]
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Recomputes the statistics from the index and the packs on disk.
    ///
    /// **An oracle and a diagnostic, never the source of
    /// [`statistics`](Self::statistics).** FR-22 requires the totals to be
    /// maintained incrementally and never scanned; this exists so the
    /// incremental path has something independent to be checked against.
    /// Incremental accounting is the classic place for slow divergence, and a
    /// total that drifts by one delete in a hundred is invisible until a user
    /// runs compaction on a figure that was wrong.
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

    /// Checks everything that must be true before any write.
    ///
    /// **FR-27 lives here.** The generation counter is a detector only if
    /// something consults it before writing, and this is the one place every
    /// write passes through. A vault in a sync folder gaining a write from
    /// another machine is an expected condition, not an anomaly; winning
    /// silently would discard it.
    fn begin_write(&self) -> Result<()> {
        if self.lock.access() == Access::ReadOnly {
            return Err(Error::Io {
                kind: std::io::ErrorKind::ReadOnlyFilesystem,
            });
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

    /// Streams one source into the packs, producing an entry that nothing yet
    /// references.
    ///
    /// **Nothing here advances a generation**, which is what makes FR-12 and
    /// FR-14 true at once: on any failure or cancellation the packs are rolled
    /// back and the index never learned of the attempt.
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
            // The hook is the only seam progress, cancellation, and the size
            // limit need. The limit is checked here rather than from the
            // source's stated length because a limit read from file metadata
            // is a limit on files, not on content: a growing file, a pipe, or
            // any `Read` that is not a file would pass it and then write past
            // the bound (FR-15, C-2).
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
    /// **Ordering is what makes FR-12 true.** Content is written and fsynced
    /// before the index generation that references it advances, and success is
    /// reported only after the index write returns. A crash between the two
    /// leaves pack bytes that no index references — garbage, reclaimed by the
    /// reconciliation of Phase 4 — and never an index entry pointing at bytes
    /// that were not durable.
    ///
    /// # Errors
    ///
    /// [`Error::LimitExceeded`] naming the limit and the actual value,
    /// [`Error::Cancelled`], [`Error::ChangedOnDisk`], or [`Error::Io`].
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

    /// Stores one file from the filesystem.
    ///
    /// **The source is opened read-only and left exactly as it was** (FR-9).
    /// Nothing in `veil-core` deletes or modifies a file outside a vault, so an
    /// interrupted or failed ingest cannot lose data.
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
    /// Symbolic links are not followed and are returned as skipped — omitting
    /// them silently would produce a vault the user believes is complete.
    ///
    /// **Progress is counted in entries, not bytes.** A folder ingest is one
    /// operation to the person watching it, and forwarding each file's byte
    /// counter would send the figure back to zero at every file — a bar that
    /// restarts is worse than no bar. A caller that wants byte-level progress
    /// for one file drives [`add_path`](Self::add_path) itself, which is the
    /// same choice §4.8 makes for verification and for the same reason.
    ///
    /// # Errors
    ///
    /// As [`add`](Self::add). A failure partway leaves the entries already
    /// committed in place; each file is its own transaction, so a folder ingest
    /// that stops has stored a prefix of the folder rather than nothing.
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
    /// **There is never a moment with zero intact versions** (HC-4). The new
    /// content is written and durable first; then one generation step
    /// simultaneously points the path at the new entry and marks the old
    /// entry's extents reclaimable. Two steps — remove then add — would create
    /// exactly the window this forbids, and it would be invisible to every test
    /// that does not fail between them.
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

        // From here on nothing can fail before the single index write, which is
        // what makes this one generation step rather than two.
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
    /// **The stored bytes remain until compaction, and this is not an
    /// oversight.** They are counted into reclaimable bytes so the figure the
    /// user is shown says so (FR-8, FR-29). Anyone tempted to truncate here
    /// should note that packs are shared between entries and append-only: the
    /// bytes cannot be removed without rewriting the pack, which is exactly
    /// what compaction is (FR-23).
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
        // `next_entry_id` is deliberately untouched. Reissuing a deleted
        // entry's identifier would let its wrapped key decrypt under a live
        // entry's nonce (§3.2, §3.3).
        self.commit()
    }

    /// Changes the vault's password (FR-4).
    ///
    /// **Only the master key's wrapping changes.** No content, no index, and no
    /// entry key is touched, which is why FR-4's completion time is independent
    /// of vault size — the property is structural rather than measured. The old
    /// password is verified before anything is written, because verifying
    /// afterwards would destroy a vault on a typo.
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
            return Err(Error::Io {
                kind: std::io::ErrorKind::ReadOnlyFilesystem,
            });
        }

        // Verified against what is on disk, not against what is in memory: the
        // question FR-4 asks is whether the caller knows the password that
        // currently opens this vault.
        let bytes = std::fs::read(self.dir.join(HEADER_FILE))?;
        let (_, master) = unlock(&bytes, old)?;

        let mut kdf_salt = [0u8; SALT_LEN];
        let mut wrap_nonce = [0u8; WRAP_NONCE_LEN];
        // A fresh salt and a fresh nonce. Reusing either would make two
        // passwords' wrappings relatable, and a reused nonce under a
        // rederivable key is a break rather than a weakness.
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

    /// Re-reads the index from disk, adopting whatever an external writer left
    /// (FR-27).
    ///
    /// **The other half of FR-27.** Detecting the change and refusing to write
    /// over it is only useful if there is a way forward, and requiring the
    /// password again to get one would make "offer to reload" a re-open in
    /// disguise. The subkeys are already held, so this costs one index read.
    ///
    /// Anything the caller had staged against the old view is stale afterwards
    /// — including entry identifiers, which is why this returns nothing and the
    /// caller re-reads [`entries`](Self::entries).
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
    /// Nothing reaches `dst` before it has authenticated, and the content hash
    /// is compared after the final chunk (FR-17). Writing to a `Write` rather
    /// than to a path is what makes S-1 structural and is a direct correction:
    /// the original Veil chose the destination itself and wrote into the
    /// working directory over the user's original.
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

    /// Extracts one entry to a path, removing the partial output on failure.
    ///
    /// **This wrapper belongs in the core, not in each frontend.** FR-17 says
    /// incomplete output must be removed rather than left looking like a valid
    /// file, and only whoever created the file can remove it —
    /// [`extract`](Self::extract) deliberately cannot, because it never learns
    /// a path. Implementing the removal twice, once per frontend, is how the
    /// two frontends come to differ, which A-4 forbids.
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
            // A truncated plaintext left on disk is indistinguishable from a
            // short file, which is precisely what HC-3 forbids. If the removal
            // itself fails there is nothing further to be done, and the
            // original failure is the one the caller needs.
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
            // An I/O failure reaching an entry's extents usually means a pack
            // is gone. §4.5 calls that total damage to *that pack*, not a
            // broken vault, and S-4 wants the pack named — so the pack is
            // named rather than folded into "content is damaged".
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
    /// **Reuses the extraction path with the output discarded**, which is the
    /// whole design of §4.8: a verification routine that re-implements the read
    /// path verifies its own re-implementation. Reuse is what makes
    /// "verification passed" mean "extraction will succeed".
    ///
    /// Failure is per entry. A failing entry is recorded and verification
    /// continues, so one damaged pack yields a complete list of what it cost
    /// rather than stopping at the first casualty (S-4).
    ///
    /// # Errors
    ///
    /// Never for a damaged entry — that is a verdict, not an error. Only if the
    /// report itself cannot be produced.
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

            // Progress is per entry rather than per byte, because the Design
            // Guideline's estimate is in time and entry counts are what a user
            // can hold in their head (§4.8).
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
/// Written beside and renamed over, so a failure partway leaves the previous
/// header intact rather than a half-written one (HC-4). The header is the one
/// file whose loss costs the whole vault, and it is small enough that the extra
/// file costs nothing.
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
