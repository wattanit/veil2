//! Typed error taxonomy (Spec §6).
//!
//! No variant carries plaintext, content, key material, or the password (HC-2)
//! — enforced by not giving any variant a field that could hold one. Entry
//! *identity* is fine and necessary: FR-33 and S-4 need failing entries named.
//!
//! No `anyhow`. The original Veil flattened every failure into one
//! string-carrying variant, which is why a wrong password and a corrupt vault
//! were indistinguishable to callers.

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

/// Which part of a vault was found damaged. Attributed to a component so a
/// partial failure reads as a list of unreadable files, not a failed vault
/// (S-4).
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

/// Every way a vault operation can fail. Each variant carries what happened
/// and what state things are left in.
///
/// Deliberately **not** `#[non_exhaustive]`. That attribute buys forward
/// compatibility for callers outside this workspace, and there are none — the
/// crate is unpublished. What it would cost is real: the command line maps
/// every variant to an exit code (Spec §5.2), and a wildcard arm is how a new
/// variant silently becomes "unexpected failure". The compiler enforcing that
/// mapping is worth more than a compatibility affordance nothing uses.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The password did not unwrap the master key, and the header is otherwise
    /// well formed (FR-2). Distinct from corruption, so a typo does not send
    /// the user looking for a damaged file.
    #[error("that password does not open this vault; the vault itself is intact")]
    WrongPassword,

    /// Not a Veil vault at all — reported before any key derivation, and never
    /// as damage (FR-2).
    #[error("this is not a Veil vault")]
    NotAVault,

    /// The format is newer than this release understands (FR-5). Guessing at an
    /// unknown format risks HC-3.
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
    /// `affected` lists every entry it costs, not the first (S-4); empty means
    /// the damage belongs to no single entry, such as the header.
    #[error("{what} is damaged; {} entr{} affected", affected.len(), if affected.len() == 1 { "y" } else { "ies" })]
    Corrupt {
        /// Which component was found damaged.
        what: Damaged,
        /// Every entry rendered unreadable by it.
        affected: Vec<EntryId>,
    },

    /// The password offered for a new vault is shorter than C-4's minimum
    /// (FR-1).
    ///
    /// Carries the minimum, never the length offered: the length of a password
    /// is a fact about the password (HC-2).
    #[error("a vault password must be at least {minimum} characters")]
    PasswordTooShort {
        /// C-4's minimum, in characters.
        minimum: usize,
    },

    /// Nothing in the vault matches what the caller asked for.
    ///
    /// Distinct from [`Corrupt`](Self::Corrupt) for the same reason
    /// [`WrongPassword`](Self::WrongPassword) is: a mistyped name and a damaged
    /// vault send a user to entirely different remedies.
    ///
    /// Carries no path, for the reason [`Io`](Self::Io) carries none — the
    /// caller supplied it and is the layer that can name it. It is also the
    /// layer allowed to: a name is index data (HC-1).
    #[error("no file in this vault matches that")]
    NotFound,

    /// The vault already holds a file at that path (FR-34).
    ///
    /// The full path is a file's identity (FR-13), so a second file under it
    /// would leave the vault unable to say which one any later operation meant.
    #[error("this vault already holds a file at that path; replace it rather than adding a second")]
    AlreadyExists,

    /// Another process holds this vault open (FR-26).
    #[error("this vault is already open somewhere else; nothing has been changed")]
    VaultInUse,

    /// The vault changed on disk since it was opened; the write was refused
    /// rather than applied over the change (FR-27).
    #[error("this vault changed on disk since it was opened; the change was not overwritten")]
    ChangedOnDisk,

    /// The vault opened read-only and a write was attempted (§4.5, §4.8).
    ///
    /// Not an I/O failure — nothing is wrong. Read-only vaults must open, or a
    /// write-protected drive would make an interrupted compaction permanent
    /// (HC-4) and verification impossible on the drive that needs it most.
    #[error("this vault is open read-only and cannot be changed; nothing has been altered")]
    ReadOnly,

    /// The storage medium became unavailable mid-operation (FR-28).
    #[error("the vault's storage is no longer reachable; the operation stopped where it was")]
    StorageUnavailable,

    /// The operation would exceed a configured limit, and carries both numbers
    /// the message has to name (FR-15).
    #[error("the {limit} limit is {allowed}; this would make it {actual}. Nothing was added")]
    LimitExceeded {
        /// Which limit.
        limit: Limit,
        /// The configured maximum.
        allowed: u64,
        /// What the operation would have produced.
        actual: u64,
    },

    /// The caller cancelled. `rolled_back` says what it left behind
    /// (FR-14, FR-19).
    #[error(
        "cancelled{}",
        if *rolled_back { "; the vault is as it was before this started" }
        else { "; the vault has changed and the change stands" }
    )]
    Cancelled {
        /// Whether the vault was returned to its prior state.
        rolled_back: bool,
    },

    /// Verification found failing entries — every one, not the first
    /// (FR-33, S-4). Veil2 stores no redundancy, so this reports loss rather
    /// than repairing it.
    #[error("{} entr{} failed verification and cannot be recovered", entries.len(), if entries.len() == 1 { "y" } else { "ies" })]
    VerificationFailed {
        /// Every entry that failed.
        entries: Vec<EntryId>,
    },

    /// An underlying I/O failure, kind only. No path: the caller supplied it
    /// and is the layer that can name it.
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
                // Nothing has been superseded yet. This arm exists so the
                // taxonomy is complete, not because it is reachable.
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
            // Authentication failure alone cannot say whether the password was
            // wrong or the data altered. Callers that can tell classify it
            // before converting; this is the default for those that cannot.
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
            // The crypto layer does not know why a hook stopped it. Callers
            // record their own reason and restore it; cancellation is the only
            // default reading that does not overstate what happened.
            C::Stopped => Self::Cancelled { rolled_back: false },
        }
    }
}
