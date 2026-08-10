//! Putting data into a vault (Spec §4.7; FR-9, FR-10, FR-11, FR-12, FR-15,
//! FR-16).

use std::io::Read;
use std::path::Path;

use crate::crypto::{CryptoError, encrypt_watched, generate_dek, generate_nonce_prefix, wrap_dek};
use crate::error::{Error, Limit, Result};
use crate::index::{Entry, EntryId};
use crate::store::EntryWriter;

use super::{Cancel, NoProgress, Progress, ProgressReport, Skipped, Unit, Vault, normalize, walk};

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
    /// (FR-12). A crash or cancellation between the two leaves an unreferenced
    /// entry file behind, never an index entry pointing at bytes that were not
    /// durable (Spec §4.5).
    ///
    /// # Errors
    ///
    /// [`Error::AlreadyExists`] if the vault already holds that path (FR-14),
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
        // would leave every later operation on that path guessing (FR-14).
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
        let entry = self.stage(id, name, folder, src, progress, cancel)?;

        self.document.next_entry_id += 1;
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
        let root_name = root_folder_name(root);

        for file in &found.files {
            let folder = join_folder(&root_name, &file.folder);
            let mut source = std::fs::File::open(&file.path)?;
            added.push(self.add(&file.name, &folder, &mut source, &mut NoProgress, cancel)?);
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

    /// Streams one source into its own file under `entries/`, producing an
    /// entry nothing yet references.
    ///
    /// `name` and `folder` are normalised to NFC here, so every entry this
    /// vault ever stores is normalised regardless of which caller reached it
    /// (§4.6) — `add` and `replace` both go through this one point.
    ///
    /// Advances no generation. A failure or cancellation leaves the entry file
    /// exactly as far as it got, as unreferenced residue (Spec §4.5) — there is
    /// no rollback to run, because nothing yet points at it.
    pub(super) fn stage(
        &self,
        id: EntryId,
        name: &str,
        folder: &str,
        src: &mut impl Read,
        progress: &mut impl Progress,
        cancel: &Cancel,
    ) -> Result<Entry> {
        let name = normalize::nfc(name);
        let folder = normalize::nfc(folder);
        if cancel.is_cancelled() {
            return Err(Error::Cancelled { rolled_back: true });
        }

        let dek = generate_dek();
        let nonce_prefix = generate_nonce_prefix();
        let max_file_size = self.limits.max_file_size;

        let mut sink = EntryWriter::create(&self.dir, id)?;

        let outcome = {
            // The size limit is checked here rather than from the source's
            // stated length: metadata is a limit on files, not on content, and
            // every non-file source would slip past it (FR-16, C-2).
            let mut stop: Option<Error> = None;
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
            encrypt_watched(&dek, &nonce_prefix, id.get(), src, &mut sink, &mut hook).map_err(|e| {
                match e {
                    CryptoError::Stopped => stop.unwrap_or(Error::Cancelled { rolled_back: true }),
                    other => Error::from(other),
                }
            })
        };

        let summary = outcome?;

        // The entry's file is durable before anything may refer to it (FR-12).
        sink.finish()?;

        Ok(Entry {
            id,
            name: name.into_owned(),
            folder: folder.into_owned(),
            size: summary.plaintext_len,
            source_mtime: now(),
            added_at: now(),
            content_hash: summary.hash,
            wrapped_dek: wrap_dek(&self.entry_wrap_key, id.get(), &dek)?,
            nonce_prefix,
            unknown: std::collections::BTreeMap::new(),
        })
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The added folder's own name (FR-10): without it, a file directly in one
/// added folder and a file directly in a *different* added folder would
/// both land at the vault's root — indistinguishable identities for two
/// files that are not the same file. `walk`'s own "relative to root, empty
/// at the root" contract is unchanged; this is `add_folder`'s to apply, not
/// `walk`'s, since other possible callers of `walk` may have no folder-add
/// of their own to name.
///
/// Falls back to empty only if `root` has no final component to name (for
/// example `/`) — a path that cannot happen through the GUI's folder picker
/// or the CLI's argument parsing, both of which require a real directory.
fn root_folder_name(root: &Path) -> String {
    root.file_name()
        .and_then(|n| n.to_str())
        .map(str::to_owned)
        .unwrap_or_default()
}

/// `root_name`, then `sub_folder` beneath it if `sub_folder` is not itself
/// empty — never a leading or trailing stray `/` for the root-level case.
fn join_folder(root_name: &str, sub_folder: &str) -> String {
    if sub_folder.is_empty() {
        root_name.to_owned()
    } else {
        format!("{root_name}/{sub_folder}")
    }
}
