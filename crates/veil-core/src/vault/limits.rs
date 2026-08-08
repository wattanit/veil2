//! Configured limits (FR-15, C-1, C-2).

/// The maximum entries one vault holds (C-1). Well above the target workload,
/// and small enough that the index can be rewritten on every change.
pub const MAX_ENTRIES_PER_VAULT: u64 = 65_536;

/// The maximum size of one stored file (C-2).
pub const MAX_FILE_SIZE: u64 = 64 * 1024 * 1024 * 1024;

/// The limits an open vault enforces.
///
/// Values rather than constants, like the pack cap: a test that has to write
/// 64 GiB to see the refusal gets marked ignored and stops running, and FR-15's
/// requirement is precisely that the refusal names both numbers.
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
