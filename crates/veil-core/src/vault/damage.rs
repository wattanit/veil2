//! Which entries a missing pack costs (Spec §4.5; S-4).
//!
//! Damage is attributable or it is not damage a user can act on. A pack that
//! the index references and the filesystem does not hold is total damage to
//! that pack — and to nothing else. The vault opens, the entries with extents
//! in it are enumerable, and every other entry stays retrievable. Refusing to
//! open would convert the loss of one pack into the loss of the whole vault,
//! which is the failure S-4 exists to reject.
//!
//! **Nothing here runs at open.** Both accessors are computed when they are
//! called, cost one existence check per referenced pack, and read no stored
//! content — so opening a vault stays a read of the header and one index slot,
//! and S-4's attribution costs nothing until somebody asks for it (S-2).
//!
//! Damage is never confused with unused space. A pack that is *missing* is
//! referenced by definition; a pack nothing references is space, and finding
//! that is `reclaim`'s job and `info`'s, both of which the user asks for.

use std::collections::BTreeSet;

use crate::index::EntryId;
use crate::store::pack_path;

use super::Vault;

impl Vault {
    /// Packs the index references that are not on disk (§4.5, S-4).
    ///
    /// Costs one existence check per referenced pack and reads no content, so
    /// it does not put vault size into the cost of opening (S-2).
    #[must_use]
    pub fn missing_packs(&self) -> Vec<u32> {
        self.referenced_packs()
            .into_iter()
            .filter(|id| !pack_path(&self.dir, *id).exists())
            .collect()
    }

    /// Every entry with an extent in a pack that is gone — the enumeration S-4
    /// requires, so a partial loss reads as a list of files rather than as a
    /// failed vault.
    #[must_use]
    pub fn unreadable_entries(&self) -> Vec<EntryId> {
        let missing: BTreeSet<u32> = self.missing_packs().into_iter().collect();
        if missing.is_empty() {
            return Vec::new();
        }
        self.document
            .entries
            .iter()
            .filter(|e| e.extents.iter().any(|x| missing.contains(&x.pack_id)))
            .map(|e| e.id)
            .collect()
    }

    /// Every pack identifier the index refers to, ascending.
    fn referenced_packs(&self) -> Vec<u32> {
        let ids: BTreeSet<u32> = self
            .document
            .entries
            .iter()
            .flat_map(|e| e.extents.iter())
            .map(|x| x.pack_id)
            .collect();
        ids.into_iter().collect()
    }
}
