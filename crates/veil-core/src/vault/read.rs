//! Getting data back out, and verifying that it is still there
//! (Spec §4.7, §4.8; FR-17, FR-18, FR-19, FR-20, FR-26).

use std::io::Write;
use std::path::Path;

use crate::crypto::{CryptoError, Dek, NONCE_PREFIX_LEN, decrypt_watched, unwrap_dek};
use crate::error::{Damaged, Error, Result};
use crate::index::{Entry, EntryId};

use super::{Cancel, NoProgress, Outcome, Progress, ProgressReport, Report, Unit, Vault, Verdict};

impl Vault {
    /// Writes one entry's content to `dst`, verified.
    ///
    /// Nothing reaches `dst` before it authenticates, and the content hash is
    /// compared after the final chunk (FR-18). Takes a `Write` rather than a
    /// path, so no destination is ever chosen in here.
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`] if no entry has that identifier,
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
            return Err(Error::NotFound);
        };
        self.read_entry(entry, dst, progress, cancel)
    }

    /// Extracts one entry to a path, removing the partial output on failure
    /// (FR-18). Lives here rather than in each frontend so the removal is
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

    /// Verifies every entry, writing nothing (FR-26, §4.8).
    ///
    /// Reuses the extraction path with the output discarded, so "verification
    /// passed" means "extraction will succeed". Failure is per entry:
    /// verification continues and returns every failure, so damage to one
    /// entry's file yields a full list of what it cost (S-3).
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

    /// The single read path: unwrap the entry's key, stream its file through
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

        // A missing or otherwise unreadable file is damage to exactly this
        // entry (Spec §4.5, S-3) — with one file per entry, there is no
        // attribution to compute; the file that failed to open already names
        // the entry it belongs to.
        let Ok(mut source) = crate::store::open_for_read(&self.dir, id) else {
            return Err(Error::Corrupt {
                what: Damaged::EntryFile,
                affected: vec![id],
            });
        };

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
            // A read failure mid-stream — the file exists but is short or
            // otherwise unreadable past its start — is damage to this entry's
            // own file, the same as a missing one (Spec §4.5).
            Err(CryptoError::Io) => Err(Error::Corrupt {
                what: Damaged::EntryFile,
                affected: vec![id],
            }),
            Err(_) => Err(Error::Corrupt {
                what: Damaged::Content,
                affected: vec![id],
            }),
        }
    }
}
