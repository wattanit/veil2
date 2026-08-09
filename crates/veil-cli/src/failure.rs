//! What a command failed with, and what the shell sees (Spec §5.2, §6).

use veil_core::Error;

/// The result type every command returns.
pub type Run<T> = std::result::Result<T, Failure>;

/// A failure, classified by what the caller should do about it.
#[derive(Debug)]
pub enum Failure {
    /// The library refused. The exit code comes from the Spec §6 class.
    Vault(Error),
    /// The invocation itself was wrong.
    Usage(String),
    /// A password was needed and there is no way to ask for one.
    NoPassword(String),
    /// Damage was found. Not an error in the library's sense — a verdict — so
    /// it carries the wording rather than a variant.
    Damage(String),
    /// No file at the path the caller named. Carries the path: the library
    /// refuses to hold a name (HC-1), and this is the layer that already has
    /// it, having been given it.
    NoSuchFile(String),
    /// A file is already at that path (FR-14).
    AlreadyThere(String),
    /// Anything else, with whatever context the operation could give it.
    Other(anyhow::Error),
}

impl Failure {
    /// The exit code, per the Spec §5.2 table.
    ///
    /// The match over [`Error`] is exhaustive by construction: that enum is
    /// deliberately not `#[non_exhaustive]`, so adding a variant without a code
    /// fails to compile rather than silently landing in 1.
    #[must_use]
    pub fn code(&self) -> u8 {
        match self {
            Self::Other(_) => 1,
            Self::Usage(_) => 2,
            Self::Damage(_) => 5,
            Self::NoPassword(_) => 12,
            Self::NoSuchFile(_) | Self::AlreadyThere(_) => 13,
            Self::Vault(e) => match e {
                Error::PasswordTooShort { .. } => 2,
                Error::WrongPassword => 3,
                Error::NotAVault | Error::FormatTooNew { .. } | Error::FormatSuperseded { .. } => 4,
                Error::Corrupt { .. } | Error::VerificationFailed { .. } => 5,
                Error::VaultInUse => 6,
                Error::ChangedOnDisk => 7,
                Error::ReadOnly => 8,
                Error::LimitExceeded { .. } => 9,
                Error::Cancelled { .. } => 10,
                Error::StorageUnavailable => 11,
                Error::NotFound | Error::AlreadyExists => 13,
                Error::Io { .. } => 1,
            },
        }
    }

    /// What to tell the user. One sentence first, technical text never.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Vault(e) => e.to_string(),
            Self::Usage(m)
            | Self::NoPassword(m)
            | Self::Damage(m)
            | Self::NoSuchFile(m)
            | Self::AlreadyThere(m) => m.clone(),
            Self::Other(e) => e.to_string(),
        }
    }
}

impl From<Error> for Failure {
    fn from(e: Error) -> Self {
        Self::Vault(e)
    }
}

impl From<anyhow::Error> for Failure {
    fn from(e: anyhow::Error) -> Self {
        Self::Other(e)
    }
}

impl From<std::io::Error> for Failure {
    fn from(e: std::io::Error) -> Self {
        Self::Other(e.into())
    }
}
