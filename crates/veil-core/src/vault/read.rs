//! Getting data back out, and verifying that it is still there
//! (Spec §4.7, §4.8; FR-16, FR-17, FR-19, FR-33).

use std::io::Write;
use std::path::Path;

use crate::crypto::{CryptoError, Dek, NONCE_PREFIX_LEN, decrypt_watched, unwrap_dek};
use crate::error::{Damaged, Error, Result};
use crate::index::{Entry, EntryId};
use crate::store::PackSource;

use super::mutate::no_such_entry;
use super::{Cancel, NoProgress, Outcome, Progress, ProgressReport, Report, Unit, Vault, Verdict};

impl Vault {
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
            return Err(no_such_entry(Some(id)));
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
            // Per entry rather than per byte (§4.8).
            progress.report(ProgressReport {
                unit: Unit::Entries,
                done: index as u64 + 1,
                total,
            });
        }

        Ok(report)
    }

    /// The single read path: unwrap the entry's key, stream its extents through
    /// the AEAD, compare the content hash. Extraction and verification both go
    /// through here, which is what makes them agree.
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
            Err(CryptoError::Io) => Err(self.attribute_io_failure(entry)),
            Err(_) => Err(Error::Corrupt {
                what: Damaged::Content,
                affected: vec![id],
            }),
        }
    }

    /// An I/O failure reaching an entry's extents usually means a pack is gone.
    /// Name the pack rather than folding it into "content is damaged" (S-4).
    fn attribute_io_failure(&self, entry: &Entry) -> Error {
        let missing = entry
            .extents
            .iter()
            .find(|x| !crate::store::pack_path(&self.dir, x.pack_id).exists());
        match missing {
            Some(extent) => crate::store::damaged_pack(extent.pack_id, vec![entry.id]),
            None => Error::Corrupt {
                what: Damaged::Content,
                affected: vec![entry.id],
            },
        }
    }
}
