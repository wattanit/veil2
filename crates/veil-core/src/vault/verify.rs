//! Whole-vault verification results (Spec §4.8; FR-33, S-4).

use crate::error::Damaged;
use crate::index::EntryId;

/// What verification found for one entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Every chunk authenticated and the content matched its recorded hash.
    Passed,
    /// The entry could not be read back intact.
    Failed(Damaged),
}

/// One entry's verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verdict {
    /// Which entry.
    pub id: EntryId,
    /// What was found.
    pub outcome: Outcome,
}

/// The result of verifying a vault. Failure is per entry, not per vault: one
/// damaged pack yields a full list of what it cost rather than stopping at the
/// first casualty (§4.8, S-4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// One verdict per entry examined, in index order.
    pub verdicts: Vec<Verdict>,
    /// Whether every entry was examined. `false` after cancellation — a
    /// partial verification is a partial answer, not a discarded one.
    pub complete: bool,
}

impl Report {
    /// Every entry that failed, by identifier. The caller turns these into
    /// names; `veil-core` keeps names out of errors (HC-1).
    #[must_use]
    pub fn failures(&self) -> Vec<EntryId> {
        self.verdicts
            .iter()
            .filter(|v| v.outcome != Outcome::Passed)
            .map(|v| v.id)
            .collect()
    }

    /// Whether every entry examined passed. Not the same as "the vault is
    /// sound" when [`complete`](Self::complete) is false.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.verdicts.iter().all(|v| v.outcome == Outcome::Passed)
    }
}
