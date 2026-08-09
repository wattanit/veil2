//! Which entries are missing their file (Spec §4.5; S-3).
//!
//! With one file per entry, damage cannot spread past its own file — there is
//! no attribution to compute, only a direct check of whether each entry's
//! file is there.
//!
//! **Nothing here runs at open.** Computed when called, one existence check
//! per entry, reading no stored content — so opening a vault stays a read of
//! the header and one index slot (S-2).

use crate::index::EntryId;
use crate::store;

use super::Vault;

impl Vault {
    /// Entries whose file is missing — the enumeration S-3 requires, so a
    /// partial loss reads as a list of files rather than as a failed vault.
    ///
    /// Costs one existence check per entry and reads no content, so it does
    /// not put vault size into the cost of opening (S-2).
    #[must_use]
    pub fn unreadable_entries(&self) -> Vec<EntryId> {
        self.document
            .entries
            .iter()
            .filter(|e| !store::exists(&self.dir, e.id))
            .map(|e| e.id)
            .collect()
    }
}
