//! Reconciling stored data against the index at open (Spec §4.5; FR-32, HC-4,
//! S-4).
//!
//! A crash leaves residue. An interrupted ingest leaves a tail of pack bytes no
//! index ever named; an interrupted reclaim leaves either the new pack the
//! index had not adopted yet or the old one it had already let go of.
//!
//! **Residue is found here and reported. It is not destroyed here.** FR-32 asks
//! for it to be discarded at open, and that is a step this refuses to take, for
//! a reason HC-4 settles: whether unreferenced bytes are residue is a *guess*,
//! and the guess can be wrong in a way that costs the user data.
//!
//! The case that decides it is the one §1 names as a motivation for the product
//! — a vault in a sync folder. A daemon replicating a vault can deliver an
//! older index before the packs that a newer one describes. Opened at that
//! moment, the vault sees bytes its index does not account for, which is
//! indistinguishable from the residue of a killed ingest. Discarding them
//! destroys content the newer index, arriving seconds later, still points at.
//! No interruption occurred and data was lost anyway, which is precisely what
//! HC-4 forbids. FR-32's own words name the target as "the residue an
//! interrupted ingest or compaction leaves behind under HC-4", so where the
//! identification is uncertain, HC-4 governs.
//!
//! What happens instead: the residue is counted into the space the user can
//! reclaim, and reclaiming it is the deliberate act FR-23 already requires for
//! recovering space. Nothing accumulates invisibly — `info` shows it — and
//! nothing is destroyed on a guess. Recorded as *Notes for Upstream*, item 7.
//!
//! **Telling residue from a deleted file's bytes needs no new field.** Both are
//! unreferenced; the statistics count what committed operations put on disk and
//! the filesystem counts what is there, so the difference is exactly the
//! residue. A delete leaves its bytes counted; a killed ingest leaves bytes
//! nothing counted.
//!
//! **Nothing here writes the index**, and that matters more than it looks. An
//! index write at open advances the generation, and the generation is FR-27's
//! whole mechanism: a vault opened from a stale copy would come away holding a
//! number higher than the newer index a daemon then delivers, and every later
//! write would sail past the check that exists to refuse it.
//!
//! Residue and damage are different things and are never confused: a pack that
//! is *missing* is referenced by definition, so it is damage, and this module
//! reports it rather than adjusting the vault to match it.

use std::collections::BTreeSet;

use crate::error::Result;
use crate::index::EntryId;
use crate::store::{pack_path, total_pack_bytes};

use super::{Access, Vault};

/// What reconciliation found when the vault was opened (FR-32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reconciled {
    /// The vault opened read-only, so nothing was examined. Reconciliation
    /// reads the filesystem and would report a change; read-only media must
    /// still open (§4.5).
    Skipped,
    /// Every byte on disk is accounted for. The ordinary case.
    Clean,
    /// Bytes on disk that no committed operation put there — what an
    /// interrupted ingest or reclaim left behind. They are counted into the
    /// space the user can reclaim rather than destroyed on the spot, and this
    /// is the report FR-32 requires instead of absorbing them silently.
    Residue {
        /// How many such bytes were found.
        bytes: u64,
    },
}

impl Reconciled {
    /// The residue found, or zero when there was none or nothing was examined.
    #[must_use]
    pub fn residue_bytes(self) -> u64 {
        match self {
            Self::Skipped | Self::Clean => 0,
            Self::Residue { bytes } => bytes,
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

    /// Finds the residue an interrupted operation left, and counts it into the
    /// space the user can reclaim.
    ///
    /// Called once, at open. Writes nothing at all — not the index, not a pack
    /// — so opening a vault stays a read, and the generation FR-27 depends on
    /// is never advanced behind the user's back.
    pub(super) fn reconcile(&mut self) -> Result<Reconciled> {
        // Reading the filesystem is all this does, but reporting a figure the
        // caller will offer to act on is pointless where acting is impossible,
        // and read-only media must open regardless (§4.5, FR-32).
        if self.lock.access() == Access::ReadOnly {
            return Ok(Reconciled::Skipped);
        }

        // File sizes, never stored content, so FR-22's prohibition on scanning
        // is untouched and open time does not follow vault size (S-2).
        let on_disk = total_pack_bytes(&self.dir)?;
        let committed = self.document.statistics.physical_bytes;

        // Less on disk than was committed means a pack is gone. That is damage,
        // not residue, and adjusting the figures to match it would be damage
        // covering its own tracks (S-4) — `missing_packs` reports it instead.
        let Some(residue) = on_disk.checked_sub(committed).filter(|n| *n > 0) else {
            return Ok(Reconciled::Clean);
        };

        // In memory only. The figures are true for this session and derived
        // again at the next open; writing them would cost FR-27 its detector.
        self.document.statistics.physical_bytes = on_disk;
        self.document.statistics.reclaimable_bytes += residue;

        Ok(Reconciled::Residue { bytes: residue })
    }
}
