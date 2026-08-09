//! The entry model (Spec §4.3).

use serde::{Deserialize, Serialize};

use crate::crypto::{HASH_LEN, NONCE_PREFIX_LEN, WRAPPED_KEY_LEN};

/// Identifies one entry within one vault.
///
/// Opaque and meaningless outside the vault that minted it. Carrying an
/// identifier rather than a name is what lets an error name a failing entry
/// without a log line reconstructing the index (HC-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

/// One stored file.
///
/// **No absolute source path is recorded, in any field.** The original Veil
/// stored one, which retained a fact about the user's machine that nothing
/// needed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Identity within this vault. Also names this entry's file:
    /// `entries/{id}.entry` (§4.1).
    pub id: EntryId,
    /// File name, NFC, UTF-8 (§4.6).
    pub name: String,
    /// Path relative to the added root — descriptive metadata, not structure
    /// (FR-8). Together with `name` this is the entry's identity for the
    /// purposes of replacement (FR-13).
    pub folder: String,
    /// Plaintext length in bytes.
    pub size: u64,
    /// Modification time of the source when it was added.
    pub source_mtime: u64,
    /// When it was added to the vault.
    pub added_at: u64,
    /// BLAKE3 of the plaintext (FR-18).
    #[serde(with = "super::byte_array")]
    pub content_hash: [u8; HASH_LEN],
    /// This entry's data key, wrapped under the entry-wrap subkey.
    #[serde(with = "super::byte_array")]
    pub wrapped_dek: [u8; WRAPPED_KEY_LEN],
    /// This entry's STREAM nonce prefix.
    #[serde(with = "super::byte_array")]
    pub nonce_prefix: [u8; NONCE_PREFIX_LEN],

    /// Fields written by a version this build does not know.
    ///
    /// **Preserved across a read and write cycle** (FR-6). This is the
    /// reader's half of the migration door that Requirements §2.2 defers and
    /// HC-5 holds open: a reader that drops what it does not understand turns
    /// a future migration from a translation into a reconstruction.
    #[serde(flatten)]
    pub unknown: std::collections::BTreeMap<String, ciborium::Value>,
}
