//! Failures originating inside the cryptographic layer.
//!
//! This module carries its own error type rather than using the crate's
//! taxonomy, so that `crypto` depends on no sibling module (Spec §1). The
//! conversion into the crate taxonomy lives in `error`, which points the
//! dependency the other way.

/// A cryptographic operation failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CryptoError {
    /// Authentication failed: the key is wrong, or the data or its associated
    /// data was altered. Which of those it is cannot be told apart here, and
    /// this layer does not guess (FR-2 is decided by the caller, which can see
    /// whether the surrounding structure is intact).
    #[error("authentication failed")]
    Authentication,

    /// Key derivation could not run with the parameters it was given.
    #[error("key derivation failed")]
    Derivation,

    /// Key-derivation parameters outside the range this build accepts.
    #[error("key-derivation parameters are out of range")]
    ParametersOutOfRange,

    /// Content authenticated chunk by chunk but did not match the hash
    /// recorded for it (FR-17).
    #[error("content does not match its recorded hash")]
    ContentHashMismatch,

    /// Reading or writing failed underneath the cryptographic layer.
    #[error("a storage operation failed")]
    Io,

    /// The caller's chunk hook asked the operation to stop.
    ///
    /// This layer does not know why — cancellation and a limit being exceeded
    /// look identical from here, and both belong to the caller. The caller
    /// records its own reason before returning it from the hook and restores it
    /// when this arrives back (A-3, FR-14, FR-15).
    #[error("stopped by the caller")]
    Stopped,
}
