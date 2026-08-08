//! Reconciling stored data against the index at open (Spec §4.5; FR-32, HC-4,
//! S-4).
//!
//! A crash leaves residue. An interrupted ingest leaves pack bytes no index
//! ever named; an interrupted reclaim leaves either the new pack the index had
//! not adopted yet or the old one it had already let go of. Both are packs that
//! nothing references, and both are removed here.
//!
//! Residue and damage are different things and are never confused: a pack that
//! is *missing* is referenced by definition, so it is damage, and this module
//! reports it rather than adjusting the vault to match it.
//!
//! **A pack that deleting emptied completely is removed here too, and that is a
//! reading rather than an oversight.** FR-32 says to discard stored data no
//! index entry references, and such a pack is exactly that. It looks at first
//! like FR-23's prohibition on automatic compaction being broken, and it is
//! not: nothing live is rewritten, no extent moves, and none of FR-23's stated
//! cost — competing for I/O, risking an interruption the user did not choose —
//! applies to unlinking a file with nothing live in it. What the product
//! promises about deleted bytes (FR-21, FR-29) is that they *may* persist until
//! space is reclaimed, so a user is never told those bytes are gone when they
//! are not; here they really are gone. The alternative — telling residue from
//! garbage — needs the index to record which packs it has ever known about,
//! and that is a format field bought to preserve bytes the user asked to be
//! rid of. Recorded as *Notes for Upstream*, item 7 of the Phase 4 to-do list.

use std::collections::BTreeSet;

use crate::error::Result;
use crate::index::EntryId;
use crate::store::{existing_pack_ids, pack_path, remove_pack};

use super::{Access, Vault};

/// What reconciliation did when the vault was opened (FR-32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reconciled {
    /// The vault opened read-only, so nothing was examined and nothing removed.
    /// Reconciliation is a write, and read-only media must still open (§4.5).
    Skipped,
    /// Reconciliation ran. Both figures are zero for an intact vault, which is
    /// the ordinary case and writes nothing.
    Done {
        /// How many packs nothing referenced and were removed.
        packs_removed: usize,
        /// The bytes those packs held, which FR-32 requires be reported rather
        /// than absorbed.
        bytes_recovered: u64,
    },
}

impl Reconciled {
    /// The bytes recovered, or zero when reconciliation was skipped.
    #[must_use]
    pub fn bytes_recovered(self) -> u64 {
        match self {
            Self::Skipped => 0,
            Self::Done {
                bytes_recovered, ..
            } => bytes_recovered,
        }
    }
}

impl Vault {
    /// What reconciliation did when this vault was opened (FR-32).
    #[must_use]
    pub fn reconciled(&self) -> Reconciled {
        self.reconciled
    }

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

    /// Removes every pack nothing references and makes the figures true again.
    ///
    /// Called once, at open. Writes nothing when there was nothing to remove:
    /// an open that changes a vault is an open that can fail, and every read of
    /// a vault would become a durability event.
    pub(super) fn reconcile(&mut self) -> Result<Reconciled> {
        // Reconciliation is a write, and read-only media must open anyway
        // (FR-32). Refusing here would turn an interrupted reclaim on a drive
        // that later became read-only into permanent data loss, which HC-4
        // forbids.
        if self.lock.access() == Access::ReadOnly {
            return Ok(Reconciled::Skipped);
        }

        let referenced: BTreeSet<u32> = self.referenced_packs().into_iter().collect();
        let orphans: Vec<u32> = existing_pack_ids(&self.dir)?
            .into_iter()
            .filter(|id| !referenced.contains(id))
            .collect();

        if orphans.is_empty() {
            return Ok(Reconciled::Done {
                packs_removed: 0,
                bytes_recovered: 0,
            });
        }

        let mut bytes_recovered = 0;
        for id in &orphans {
            bytes_recovered += remove_pack(&self.dir, *id)?;
        }

        // The statistics are incremental by FR-22, and a crash is exactly the
        // event that breaks an incremental counter: bytes written that no
        // commit learned of, or a pack written off before it was removed. Which
        // of those happened is not knowable after the fact, so the totals are
        // set to what is actually on disk. That reads file sizes, never stored
        // content, so FR-22's prohibition on scanning is untouched and open
        // time still does not follow vault size (S-2).
        //
        // Not done when a referenced pack is missing: those bytes are damage,
        // not residue, and writing a smaller vault into the index would be
        // damage covering its own tracks (S-4).
        if self.missing_packs().is_empty() {
            let counted = self.recount_statistics()?;
            self.document.statistics.physical_bytes = counted.physical_bytes;
            self.document.statistics.reclaimable_bytes = counted.reclaimable_bytes;
        }

        self.commit()?;
        Ok(Reconciled::Done {
            packs_removed: orphans.len(),
            bytes_recovered,
        })
    }
}
