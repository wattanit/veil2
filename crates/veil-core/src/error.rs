//! Typed error taxonomy (Technical Specification §6).
//!
//! Two prohibitions govern this module, and they are different rules:
//!
//! - **No error, `Display`, or `Debug` output contains plaintext, file
//!   content, key material, or the password** (HC-2). Enforced by giving no
//!   variant a field that could carry one.
//! - **Logging never records entry names, folder metadata, or content**
//!   (HC-1). That is the logging guard's rule, not this module's.
//!
//! Errors therefore *may* carry entry identity, and must: FR-33 and S-4
//! require failing entries to be named, and an error that cannot say which
//! entry failed cannot satisfy them.
//!
//! `anyhow` is not used here. The original Veil converted every failure into
//! one string-carrying variant, which is why a wrong password and a corrupted
//! vault were indistinguishable to callers — the condition FR-2 forbids.

use crate::index::EntryId;

/// Result type for every fallible operation in this crate.
pub type Result<T> = core::result::Result<T, Error>;

/// Which limit an operation would have exceeded (FR-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    /// Entries in one vault (C-1).
    EntriesPerVault,
    /// Size of one stored file (C-2).
    FileSize,
}

impl core::fmt::Display for Limit {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EntriesPerVault => f.write_str("entries per vault"),
            Self::FileSize => f.write_str("file size"),
        }
    }
}

/// Which part of a vault was found damaged.
///
/// Damage is attributed to a component so that a partial failure is presented
/// as a list of unreadable files rather than as a failure of the vault (S-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Damaged {
    /// The plaintext header (§4.2).
    Header,
    /// One index slot; the other may still authenticate (§4.4).
    IndexSlot,
    /// Both index slots. The vault cannot be opened.
    BothIndexSlots,
    /// One pack file (§4.5).
    Pack {
        /// Identifier of the affected pack.
        id: u32,
    },
    /// An entry's stored content failed chunk authentication (§3.3).
    Content,
    /// An entry decrypted and authenticated but did not match its recorded
    /// content hash (FR-17).
    ContentHash,
}

impl core::fmt::Display for Damaged {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Header => f.write_str("the vault header"),
            Self::IndexSlot => f.write_str("one index slot"),
            Self::BothIndexSlots => f.write_str("both index slots"),
            Self::Pack { id } => write!(f, "pack {id:06}"),
            Self::Content => f.write_str("stored content"),
            Self::ContentHash => f.write_str("a content hash"),
        }
    }
}

/// Every way a vault operation can fail.
///
/// Each variant carries the state fact the Design Guideline's three-part
/// message needs (§4.2): what happened, what state things are in, and enough
/// for the caller to say what can be done.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The password did not unwrap the master key, and the header is
    /// otherwise well formed (FR-2).
    ///
    /// Distinct from every corruption variant so that a user with a typo is
    /// not sent to look for a damaged file.
    #[error("that password does not open this vault; the vault itself is intact")]
    WrongPassword,

    /// The target is not a Veil vault at all (FR-2, §4.2).
    ///
    /// Reported before any key derivation is attempted, and never as damage.
    #[error("this is not a Veil vault")]
    NotAVault,

    /// The vault's format is newer than this release understands (FR-5).
    ///
    /// Refusing is correct: guessing at an unknown format risks HC-3.
    #[error(
        "this vault needs format version {required}; this release understands \
         up to {supported}. The vault is unchanged"
    )]
    FormatTooNew {
        /// Format version the vault records as required to read it.
        required: u16,
        /// Highest format version this release understands.
        supported: u16,
    },

    /// The vault's format is older than the current one and this release no
    /// longer reads it (FR-30).
    #[error(
        "this vault uses format version {version}, which this release no \
         longer reads; the last release able to read it is {last_supported_by}"
    )]
    FormatSuperseded {
        /// Format version the vault uses.
        version: u16,
        /// Application version that last supported reading it.
        last_supported_by: &'static str,
    },

    /// Stored data was altered, truncated, reordered, or substituted (HC-3).
    ///
    /// `affected` carries every entry the damage costs, not the first (S-4).
    /// An empty `affected` means the damage is to a component that belongs to
    /// no single entry, such as the header.
    #[error("{what} is damaged; {} entr{} affected", affected.len(), if affected.len() == 1 { "y" } else { "ies" })]
    Corrupt {
        /// Which component was found damaged.
        what: Damaged,
        /// Every entry rendered unreadable by it.
        affected: Vec<EntryId>,
    },

    /// Another process holds this vault open (FR-26).
    #[error("this vault is already open somewhere else; nothing has been changed")]
    VaultInUse,

    /// The vault changed on disk since it was opened (FR-27).
    ///
    /// The write was refused rather than applied over the change.
    #[error("this vault changed on disk since it was opened; the change was not overwritten")]
    ChangedOnDisk,

    /// The storage medium became unavailable mid-operation (FR-28).
    #[error("the vault's storage is no longer reachable; the operation stopped where it was")]
    StorageUnavailable,

    /// The operation would exceed a configured limit (FR-15).
    ///
    /// Carries both numbers the message must name.
    #[error("the {limit} limit is {allowed}; this would make it {actual}. Nothing was added")]
    LimitExceeded {
        /// Which limit.
        limit: Limit,
        /// The configured maximum.
        allowed: u64,
        /// What the operation would have produced.
        actual: u64,
    },

    /// The caller cancelled the operation (FR-14, FR-19).
    ///
    /// `rolled_back` states what the cancellation left behind, which is the
    /// state fact the message must carry.
    #[error(
        "cancelled{}",
        if *rolled_back { "; the vault is as it was before this started" }
        else { "; the vault has changed and the change stands" }
    )]
    Cancelled {
        /// Whether the vault was returned to its prior state.
        rolled_back: bool,
    },

    /// A whole-vault verification found failing entries (FR-33, S-4).
    ///
    /// Carries every failing entry, not just the first. Veil2 stores no
    /// redundancy, so this reports what is already lost and repairs nothing.
    #[error("{} entr{} failed verification and cannot be recovered", entries.len(), if entries.len() == 1 { "y" } else { "ies" })]
    VerificationFailed {
        /// Every entry that failed.
        entries: Vec<EntryId>,
    },

    /// An underlying I/O failure.
    ///
    /// Carries the kind only. A path is deliberately not carried: an ingest
    /// source path is a fact about the user's machine that no error needs, and
    /// the caller that supplied the path is the layer that can name it.
    #[error("a storage operation failed ({kind})")]
    Io {
        /// The kind of I/O failure.
        kind: std::io::ErrorKind,
    },
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io { kind: e.kind() }
    }
}

impl From<crate::format::HeaderError> for Error {
    fn from(e: crate::format::HeaderError) -> Self {
        use crate::format::HeaderError as H;
        match e {
            H::NotAVault => Self::NotAVault,
            H::TooNew {
                required,
                supported,
            } => Self::FormatTooNew {
                required,
                supported,
            },
            H::Superseded { version } => Self::FormatSuperseded {
                version,
                // No format version has been superseded yet, and support is
                // not withdrawn while the migration path of Requirements §2.2
                // remains unbuilt. This arm exists so the taxonomy is complete,
                // not because it is reachable.
                last_supported_by: env!("CARGO_PKG_VERSION"),
            },
            H::Damaged => Self::Corrupt {
                what: Damaged::Header,
                affected: Vec::new(),
            },
        }
    }
}

impl From<crate::crypto::CryptoError> for Error {
    fn from(e: crate::crypto::CryptoError) -> Self {
        use crate::crypto::CryptoError as C;
        match e {
            // Authentication failure alone does not say whether the password
            // was wrong or the data was altered. Every caller that can tell
            // the difference classifies it before converting; this arm is the
            // conservative default for callers that cannot.
            C::Authentication => Self::WrongPassword,
            C::Derivation | C::ParametersOutOfRange => Self::Corrupt {
                what: Damaged::Header,
                affected: Vec::new(),
            },
            C::ContentHashMismatch => Self::Corrupt {
                what: Damaged::ContentHash,
                affected: Vec::new(),
            },
            C::Io => Self::Io {
                kind: std::io::ErrorKind::Other,
            },
            // The crypto layer does not know why a caller's hook stopped it.
            // Every caller that uses a hook records its own reason and restores
            // it before converting; this arm is the conservative default for a
            // caller that does not, and cancellation is the only reading that
            // does not overstate what happened.
            C::Stopped => Self::Cancelled { rolled_back: false },
        }
    }
}
