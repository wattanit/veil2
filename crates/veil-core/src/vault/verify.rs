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

/// The result of verifying a vault.
///
/// **Failure is per entry, not per vault** (§4.8). A failing entry is recorded
/// and verification continues, so one damaged pack yields a complete list of
/// what it cost rather than stopping at the first casualty — the attribution
/// S-4 requires.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// One verdict per entry examined, in index order.
    pub verdicts: Vec<Verdict>,
    /// Whether every entry was examined.
    ///
    /// `false` after cancellation. A partial verification is a partial answer,
    /// not a discarded one: discarding it would make cancelling cost the user
    /// everything they had already waited for.
    pub complete: bool,
}

impl Report {
    /// Every entry that failed, by identifier.
    ///
    /// This is what FR-33 requires be reported by name, and what the caller
    /// turns into names — `veil-core` does not put entry names in errors
    /// (HC-1).
    #[must_use]
    pub fn failures(&self) -> Vec<EntryId> {
        self.verdicts
            .iter()
            .filter(|v| v.outcome != Outcome::Passed)
            .map(|v| v.id)
            .collect()
    }

    /// Whether every entry examined passed.
    ///
    /// **Not the same as "the vault is sound"** when [`complete`](Self::complete)
    /// is false: an unexamined entry has no verdict, and reporting one would be
    /// a guess.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.verdicts.iter().all(|v| v.outcome == Outcome::Passed)
    }
}
