//! Public API: create, open, ingest, extract, replace, delete, verify
//! (Spec §2, §5.1). Nothing here needs a terminal or a prompt (A-1).
//!
//! `name` and `folder` are normalised to NFC wherever a caller supplies one —
//! `normalize` — so comparison (`find`, `add`, `replace`) is exact equality on
//! an already-normalised form (Spec §4.6).
//!
//! This file holds the type and the read-only accessors. The operations are
//! split by what they do to a vault: `session` opens and closes one and owns
//! the password, `ingest` puts data in, `mutate` changes what is already there,
//! `read` gets data out, and `damage` says which entry a missing file costs.
//!
//! **Opening a vault reads the header and one index slot, and does nothing
//! else.** It writes nothing, and it does not walk `entries/`. Both matter: a
//! write at open advances the generation that FR-24 detects external change
//! with, and a walk puts vault size into the cost of an open (S-2).

mod damage;
mod ingest;
mod limits;
mod lock;
mod mutate;
mod normalize;
mod progress;
mod read;
mod session;
mod verify;
mod walk;

pub use ingest::FolderOutcome;
pub use limits::{Limits, MAX_ENTRIES_PER_VAULT, MAX_FILE_SIZE};
pub use lock::{Access, LOCK_FILE, VaultLock};
pub use progress::{Cancel, NoProgress, Progress, ProgressReport, Unit};
pub use session::HEADER_FILE;
pub use verify::{Outcome, Report, Verdict};
pub use walk::{Found, SkipReason, Skipped, Walk, walk};

use std::path::PathBuf;

use crate::crypto::{EntryWrapKey, IndexKey};
use crate::error::{Error, Result};
use crate::format::Header;
use crate::index::{Entry, IndexDocument, Statistics};

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

    /// The complete index, served from memory (FR-7).
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.document.entries
    }

    /// The entry at one full path — folder and name together (FR-13, §4.6).
    ///
    /// Identity is the full path: matching on name alone would let an ingest
    /// into one folder overwrite a file in another. `folder` and `name` are
    /// normalised before comparison, so either spelling of the same visible
    /// name finds it (§4.6).
    #[must_use]
    pub fn find(&self, folder: &str, name: &str) -> Option<&Entry> {
        let folder = normalize::nfc(folder);
        let name = normalize::nfc(name);
        self.document
            .entries
            .iter()
            .find(|e| e.folder.as_str() == folder.as_ref() && e.name.as_str() == name.as_ref())
    }

    /// The vault's totals, derived from the resident entry list on every call
    /// rather than maintained separately (FR-7, Spec §4.3, §5.1).
    ///
    /// Cheap at C-1's scale — summing at most 65,536 in-memory entries costs
    /// microseconds — so there is nothing here to cache, and nothing that can
    /// drift from what `entries()` actually holds.
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        Statistics::from_entries(&self.document.entries)
    }

    /// The index generation — the external-modification detector (FR-24).
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.document.generation
    }

    /// The parsed header, for provenance and diagnostics.
    #[must_use]
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// The one place every write passes through, so FR-24's check lives here:
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
