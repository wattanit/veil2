//! Public API: create, open, ingest, extract, replace, delete, verify, reclaim
//! (Spec §2, §5.1). Nothing here needs a terminal or a prompt (A-1).
//!
//! Not built yet: NFC name normalisation (Phase 5) — so name comparison here is
//! exact on stored bytes.
//!
//! This file holds the type and the read-only accessors. The operations are
//! split by what they do to a vault: `session` opens and closes one and owns
//! the password, `ingest` puts data in, `mutate` changes what is already there,
//! `read` gets data out, `reclaim` recovers the space `mutate` and a crash left
//! behind, and `damage` says which entries a missing pack costs.
//!
//! **Opening a vault reads the header and one index slot, and does nothing
//! else.** It writes nothing, and it does not walk the packs directory. Both
//! matter: a write at open advances the generation that FR-27 detects external
//! change with, and a walk puts vault size into the cost of an open (S-2).
//! Finding space a crash left behind belongs to `reclaim` and to `info`, which
//! the user asks for.

mod damage;
mod ingest;
mod limits;
mod lock;
mod mutate;
mod progress;
mod read;
mod reclaim;
mod session;
mod verify;
mod walk;

pub use ingest::FolderOutcome;
pub use limits::{Limits, MAX_ENTRIES_PER_VAULT, MAX_FILE_SIZE};
pub use lock::{Access, LOCK_FILE, VaultLock};
pub use progress::{Cancel, NoProgress, Progress, ProgressReport, Unit};
pub use reclaim::Reclaimed;
pub use session::HEADER_FILE;
pub use verify::{Outcome, Report, Verdict};
pub use walk::{Found, SkipReason, Skipped, Walk, walk};

use std::path::PathBuf;

use crate::crypto::{EntryWrapKey, IndexKey};
use crate::error::{Error, Result};
use crate::format::Header;
use crate::index::{Entry, IndexDocument, Statistics};
use crate::store::total_pack_bytes;

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
    /// Closes the vault, releasing its lock and zeroising its keys (FR-3).
    ///
    /// Takes `self` rather than setting a flag, so a closed vault is not
    /// reachable as a value.
    pub fn lock(self) {
        drop(self);
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

    /// Recomputes the statistics from what is actually on disk.
    ///
    /// **Never the source of [`statistics`](Self::statistics)**, which FR-22
    /// requires to be incremental and available the instant a vault opens. This
    /// walks the packs directory, so its cost follows vault size and it belongs
    /// only where a user asked for it: reporting the figures, reclaiming space,
    /// and checking a recount in a test.
    ///
    /// The difference between the two is exactly the space an interrupted
    /// operation left behind. The incremental figures count what committed
    /// operations put on disk; this counts what is there. A crash leaves bytes
    /// nothing committed, so this reports more — and that surplus is space the
    /// user can reclaim.
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

    /// Advances the generation and writes the index. The single commit point
    /// for every mutation.
    fn commit(&mut self) -> Result<()> {
        self.document.generation += 1;
        crate::index::write(&self.dir, &self.index_key, &self.document)
    }
}
