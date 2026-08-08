//! Configured limits (FR-15, C-1, C-2).

/// The maximum entries one vault holds (C-1).
///
/// Chosen well above the media-library workload Veil2 targets while keeping the
/// index small enough to rewrite atomically on every change.
pub const MAX_ENTRIES_PER_VAULT: u64 = 65_536;

/// The maximum size of one stored file (C-2).
pub const MAX_FILE_SIZE: u64 = 64 * 1024 * 1024 * 1024;

/// The limits an open vault enforces.
///
/// **Values rather than constants, for the same reason the pack cap is.** A
/// test that must reach C-1's 65,536 entries or C-2's 64 GiB to exercise the
/// refusal path gets marked ignored within a month, and FR-15's requirement —
/// that the refusal *names both numbers* — is exactly the kind that rots
/// unobserved. The defaults are the configuration; the parameter is how the
/// refusal is testable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Maximum entries in the vault (C-1).
    pub max_entries: u64,
    /// Maximum size of one entry's content (C-2).
    pub max_file_size: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entries: MAX_ENTRIES_PER_VAULT,
            max_file_size: MAX_FILE_SIZE,
        }
    }
}
