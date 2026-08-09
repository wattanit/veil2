//! Putting data into a vault (Spec §4.7; FR-9, FR-10, FR-11, FR-12, FR-14,
//! FR-15).

use std::io::Read;
use std::path::Path;

use crate::crypto::{CryptoError, encrypt_watched, generate_dek, generate_nonce_prefix, wrap_dek};
use crate::error::{Error, Limit, Result};
use crate::index::{Entry, EntryId};
use crate::store::PackSink;

use super::{Cancel, NoProgress, Progress, ProgressReport, Skipped, Unit, Vault, walk};

/// One entry written to the packs and not yet referenced by the index.
///
/// Carries what the commit needs and nothing the caller has to recompute: the
/// entry, what it cost on disk, and where the pack counter now stands.
pub(super) struct Staged {
    /// The entry, complete but unreferenced.
    pub entry: Entry,
    /// Bytes it added to the packs.
    pub ciphertext_len: u64,
    /// What `next_pack_id` must become when these extents are adopted (§4.3).
    pub next_pack_id: u32,
}

/// What a folder ingest did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FolderOutcome {
    /// Every entry added, in the order stored.
    pub added: Vec<EntryId>,
    /// Every path the walk declined, with its reason (FR-11).
    pub skipped: Vec<Skipped>,
}

impl Vault {
    /// Stores one source's content under the given name and folder.
    ///
    /// Content is written and fsynced before the index generation that names it
    /// advances, and success is reported only after the index write returns
    /// (FR-12). A crash between the two leaves unreferenced pack bytes, never
    /// an index entry pointing at bytes that were not durable.
    ///
    /// # Errors
    ///
    /// [`Error::AlreadyExists`] if the vault already holds that path (FR-34),
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

        // The full path is a file's identity (FR-13), so a second file under it
        // would leave every later operation on that path guessing (FR-34).
        if self.find(folder, name).is_some() {
            return Err(Error::AlreadyExists);
        }

        let count = self.document.entries.len() as u64;
        if count >= self.limits.max_entries {
            return Err(Error::LimitExceeded {
                limit: Limit::EntriesPerVault,
                allowed: self.limits.max_entries,
                actual: count + 1,
            });
        }

        let id = EntryId::new(self.document.next_entry_id);
        let staged = self.stage(id, name, folder, src, progress, cancel)?;

        self.document.next_entry_id += 1;
        // Never lowered: an identifier this vault has handed out is spent, and
        // the counter is what keeps it that way (§4.3).
        self.document.next_pack_id = self.document.next_pack_id.max(staged.next_pack_id);
        self.document.statistics.entry_count += 1;
        self.document.statistics.logical_bytes += staged.entry.size;
        self.document.statistics.physical_bytes += staged.ciphertext_len;
        self.document.entries.push(staged.entry);
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

    /// Streams one source into the packs, producing an entry nothing yet
    /// references.
    ///
    /// Advances no generation, so a failure or cancellation rolls the packs back
    /// and the index never learned of the attempt. Shared with `replace`.
    pub(super) fn stage(
        &self,
        id: EntryId,
        name: &str,
        folder: &str,
        src: &mut impl Read,
        progress: &mut impl Progress,
        cancel: &Cancel,
    ) -> Result<Staged> {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled { rolled_back: true });
        }

        let dek = generate_dek();
        let nonce_prefix = generate_nonce_prefix();
        let max_file_size = self.limits.max_file_size;

        let mut sink = PackSink::open(&self.dir, self.pack_cap, self.document.next_pack_id)?;
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
                    // A `Read` has no length to promise, so no total is
                    // reported for an ingest.
                    total: None,
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

        // Read before the sink is consumed; the caller stores it in the same
        // commit that adopts these extents (§4.3).
        let next_pack_id = sink.next_pack_id_after();
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
        Ok(Staged {
            entry,
            ciphertext_len: summary.ciphertext_len,
            next_pack_id,
        })
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
