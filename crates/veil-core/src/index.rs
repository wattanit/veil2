//! Entry model, atomic index persistence, and statistics (Spec §4.3, §4.4).
//!
//! Phase 0 defines only the entry identifier, which the error taxonomy needs
//! in order to name affected entries (S-4). The index document, its CBOR
//! serialisation, and the double-buffered slots arrive in Phase 1.

/// Identifies one entry within one vault.
///
/// Opaque and meaningless outside the vault that minted it. Carrying an
/// identifier rather than a name is what lets an error name a failing entry
/// without a log line reconstructing the index (HC-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryId(u64);

impl EntryId {
    /// Wraps a raw identifier.
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// The raw identifier.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl core::fmt::Display for EntryId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "#{}", self.0)
    }
}
