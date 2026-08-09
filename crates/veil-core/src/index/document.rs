//! The index document and its serialisation (Spec §4.3).
//!
//! One CBOR document, encrypted whole under the index subkey. CBOR rather than
//! a compact encoding because it tolerates unknown fields, so a reader can
//! preserve what it does not understand.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::entry::Entry;

/// The index layout version, distinct from the vault's format version.
pub const CURRENT_INDEX_VERSION: u16 = 1;

/// Totals a user needs to decide whether compaction is worth running (FR-8).
/// Maintained incrementally, never scanned — deriving reclaimable space by
/// scanning would cost more than the compaction it advises (FR-22).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Statistics {
    /// Number of entries.
    pub entry_count: u64,
    /// Plaintext bytes stored.
    pub logical_bytes: u64,
    /// Bytes occupied on disk.
    pub physical_bytes: u64,
    /// Bytes compaction would recover.
    pub reclaimable_bytes: u64,
}

/// The whole index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexDocument {
    /// Layout version of this document.
    pub index_version: u16,
    /// Advanced by exactly one per committed mutation. The
    /// external-modification detector (FR-27), and what decides which slot a
    /// read takes (§4.4).
    pub generation: u64,
    /// The totals of FR-8.
    pub statistics: Statistics,

    /// The next identifier to issue, never decreasing.
    ///
    /// **Stored, not computed, and that is a cryptographic requirement.** The
    /// entry identifier is bound into the DEK-wrapping nonce and the content
    /// associated data (§3.2, §3.3). Deriving it from the highest *live* entry
    /// would reissue a deleted entry's identifier, and that entry's wrapped key
    /// would then decrypt under a live one's nonce. The counter has to outlive
    /// the entries it counted.
    ///
    /// `#[serde(default)]` so an older document still reads; the vault repairs
    /// the value upward from its live entries on load.
    #[serde(default)]
    pub next_entry_id: u64,

    /// The next pack identifier to issue, never decreasing.
    ///
    /// **Stored for the same reason `next_entry_id` is.** Allocating one above
    /// the highest pack on disk is safe until reclaiming space removes a pack
    /// that is entirely dead: if that pack held the highest identifier, the
    /// next allocation takes the number back. A stale index — the older of the
    /// two slots, or an older copy a sync daemon delivers late — then names a
    /// pack whose bytes are now a different pack's. That fails authentication
    /// and is reported as damage (HC-3), so nothing wrong is ever returned as
    /// content; but it reports damage where there is none, which sends a user
    /// looking for a corrupted vault they do not have (FR-2).
    ///
    /// `#[serde(default)]` so an older document still reads; the vault repairs
    /// the value upward from the packs its entries reference on load.
    #[serde(default)]
    pub next_pack_id: u32,

    /// Every entry.
    pub entries: Vec<Entry>,

    /// Fields written by a version this build does not know, preserved across
    /// a read and write cycle (FR-30).
    #[serde(flatten)]
    pub unknown: BTreeMap<String, ciborium::Value>,
}

impl IndexDocument {
    /// An empty index at generation zero.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            index_version: CURRENT_INDEX_VERSION,
            generation: 0,
            statistics: Statistics::default(),
            next_entry_id: 1,
            next_pack_id: 1,
            entries: Vec::new(),
            unknown: BTreeMap::new(),
        }
    }

    /// Serialises to CBOR.
    ///
    /// # Errors
    ///
    /// Fails if a value cannot be represented, which for this model means a
    /// preserved unknown field that is itself malformed.
    pub fn to_cbor(&self) -> Result<Vec<u8>, IndexFormatError> {
        let mut out = Vec::new();
        ciborium::into_writer(self, &mut out).map_err(|_| IndexFormatError::Malformed)?;
        Ok(out)
    }

    /// Parses CBOR. Total: no input panics, hangs, or allocates without bound.
    ///
    /// # Errors
    ///
    /// [`IndexFormatError::Malformed`] on anything the model cannot accept.
    pub fn from_cbor(bytes: &[u8]) -> Result<Self, IndexFormatError> {
        ciborium::from_reader(bytes).map_err(|_| IndexFormatError::Malformed)
    }
}

/// The index document could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IndexFormatError {
    /// The bytes authenticated but are not a document this build can read.
    #[error("the index is malformed")]
    Malformed,
}
