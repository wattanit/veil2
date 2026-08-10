//! Structured errors for the command layer (Design §4.2, P6.0.a).
//!
//! The frontend needs to know *which* condition occurred, not only read
//! English built for a person — Design §4.2's three-part presentation and
//! §4.3's per-condition responses both branch on this. `kind` is checked
//! exhaustively against `veil_core::Error`'s variants: a variant added there
//! without a matching arm here fails to compile, which is the point — a
//! condition Design has a designed response for is worth nothing if the
//! frontend cannot tell it apart from any other string.

use veil_core::Error;

/// What crosses the Tauri IPC boundary in place of a bare `String` error.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorInfo {
    /// One of `veil_core::Error`'s variant names, for the frontend to branch
    /// on (Design §4.3's per-condition responses).
    pub kind: &'static str,
    /// The variant's own `Display` text — human language, never internals
    /// (Spec §6).
    pub message: String,
}

impl From<Error> for ErrorInfo {
    fn from(error: Error) -> Self {
        let kind = match &error {
            Error::WrongPassword => "WrongPassword",
            Error::NotAVault => "NotAVault",
            Error::FormatTooNew { .. } => "FormatTooNew",
            Error::FormatSuperseded { .. } => "FormatSuperseded",
            Error::Corrupt { .. } => "Corrupt",
            Error::PasswordTooShort { .. } => "PasswordTooShort",
            Error::NotFound => "NotFound",
            Error::AlreadyExists => "AlreadyExists",
            Error::VaultInUse => "VaultInUse",
            Error::ChangedOnDisk => "ChangedOnDisk",
            Error::ReadOnly => "ReadOnly",
            Error::StorageUnavailable => "StorageUnavailable",
            Error::LimitExceeded { .. } => "LimitExceeded",
            Error::Cancelled { .. } => "Cancelled",
            Error::VerificationFailed { .. } => "VerificationFailed",
            Error::Io { .. } => "Io",
        };
        Self {
            kind,
            message: error.to_string(),
        }
    }
}

/// A failure with no `veil_core::Error` behind it — a bad argument, a lock
/// poisoned, a path with no file name. `kind` is `"Internal"`: there is no
/// designed per-condition response for these, only the generic one.
pub fn internal(message: impl Into<String>) -> ErrorInfo {
    ErrorInfo {
        kind: "Internal",
        message: message.into(),
    }
}
