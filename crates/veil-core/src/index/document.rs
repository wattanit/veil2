//! The index document and its serialisation (Spec §4.3).
//!
//! One CBOR document (RFC 8949), encrypted whole under the index subkey.
//!
//! CBOR is chosen over a compact non-self-describing encoding because it
//! tolerates unknown fields, which is what makes the deferred migration path
//! of Requirements §2.2 tractable — a reader can recognise and preserve what
//! it does not understand.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::entry::Entry;

/// The index layout version, distinct from the vault's format version.
pub const CURRENT_INDEX_VERSION: u16 = 1;

/// Totals a user needs in order to decide whether compaction is worth running
/// (FR-8).
///
/// **Maintained incrementally, never scanned** (FR-22). Deriving reclaimable
/// space by scanning hundreds of gigabytes would cost more than the compaction
/// it is meant to advise.
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
    /// Monotonic counter, advanced by exactly one per committed mutation.
    ///
    /// The external-modification detector (FR-27), and what decides which slot
    /// a read takes (§4.4).
    pub generation: u64,
    /// The totals of FR-8.
    pub statistics: Statistics,

    /// The next identifier to issue, never decreasing.
    ///
    /// **This is a cryptographic field wearing bookkeeping clothes.** The entry
    /// identifier is bound into the DEK-wrapping nonce and into the content
    /// associated data (§3.2, §3.3). Deriving the next identifier from the
    /// highest *live* one would reissue the identifier of a deleted entry, and
    /// a wrapped key from the dead entry would then decrypt under a live one's
    /// nonce. The counter must therefore outlive the entries it counted, which
    /// means it is stored rather than computed.
    ///
    /// `#[serde(default)]` so a document written before this field existed
    /// still reads; the vault repairs the value upward from its live entries on
    /// load, which is correct for every case except a vault whose highest entry
    /// was deleted before this field existed. No such vault has been released.
    #[serde(default)]
    pub next_entry_id: u64,

    /// Every entry.
    pub entries: Vec<Entry>,

    /// Document-level fields written by a version this build does not know,
    /// preserved across a read and write cycle (FR-30).
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

    /// Parses CBOR.
    ///
    /// # Errors
    ///
    /// [`IndexFormatError::Malformed`] on anything the model cannot accept.
    /// The parser is total: no input panics, hangs, or allocates without
    /// bound (P1.7.e).
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
