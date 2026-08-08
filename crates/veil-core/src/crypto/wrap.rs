//! Master-key wrapping (Spec §3.1; HC-5, HC-7, A-6).
//!
//! The password protects a master key which protects everything else, so a
//! password change rewrites 32 bytes rather than the vault (FR-4), and further
//! unwrap paths can be added later without re-encrypting content.
//!
//! **The whole preceding header is the associated data.** Any tampering with
//! the format version, the algorithm identifier, the cost parameters, or the
//! salt therefore makes the unwrap fail. That is why the header needs no
//! separate MAC of its own, and it is what closes the parameter-downgrade
//! path: an attacker who lowers the recorded cost to 1 does not get a cheaper
//! target, they get a header that no longer authenticates.
//!
//! There is exactly one unwrap path (HC-7). No escrow slot, no second
//! wrapping, no export. Unrecoverability is a property of the hierarchy rather
//! than a policy layered over it, so there is nothing to disable and nothing
//! to forget to remove.

use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305};

use super::error::CryptoError;
use super::keys::{KEY_LEN, Kek, MasterKey};

/// Length of the nonce used to wrap the master key.
pub const WRAP_NONCE_LEN: usize = 24;

/// Length of the wrapped master key: 32 key bytes plus a 16-byte tag.
pub const WRAPPED_KEY_LEN: usize = KEY_LEN + 16;

/// Generates a fresh master key from the operating system's CSPRNG.
///
/// Never derived from the password (A-6): that is what makes a password change
/// independent of vault size, and what stops two vaults sharing a password
/// from sharing content keys.
#[must_use]
pub fn generate_master_key() -> MasterKey {
    let mut bytes = [0u8; KEY_LEN];
    getrandom::fill(&mut bytes).unwrap_or_else(|_| {
        // A CSPRNG failure is not something to paper over with a weaker
        // source. Aborting is the honest response: the alternative is a vault
        // whose master key came from somewhere unknown.
        std::process::abort()
    });
    MasterKey::from_bytes(bytes)
}

/// Wraps the master key under the key-encryption key.
///
/// `header_prefix` is every header byte preceding the wrapped key, and is
/// bound as associated data.
///
/// # Errors
///
/// Fails only if the AEAD itself refuses the input.
pub fn wrap_master_key(
    kek: &Kek,
    nonce: &[u8; WRAP_NONCE_LEN],
    header_prefix: &[u8],
    master: &MasterKey,
) -> Result<[u8; WRAPPED_KEY_LEN], CryptoError> {
    let cipher = XChaCha20Poly1305::new(kek.expose().into());
    let sealed = cipher
        .encrypt(
            nonce.into(),
            Payload {
                msg: master.expose(),
                aad: header_prefix,
            },
        )
        .map_err(|_| CryptoError::Authentication)?;

    let mut out = [0u8; WRAPPED_KEY_LEN];
    if sealed.len() != WRAPPED_KEY_LEN {
        return Err(CryptoError::Authentication);
    }
    out.copy_from_slice(&sealed);
    Ok(out)
}

/// Unwraps the master key, authenticating the header in the process.
///
/// # Errors
///
/// [`CryptoError::Authentication`] when the password is wrong *or* the header
/// was altered. This layer cannot tell those apart and does not try; the
/// caller decides, because only the caller can see whether the surrounding
/// structure is intact (FR-2).
pub fn unwrap_master_key(
    kek: &Kek,
    nonce: &[u8; WRAP_NONCE_LEN],
    header_prefix: &[u8],
    wrapped: &[u8; WRAPPED_KEY_LEN],
) -> Result<MasterKey, CryptoError> {
    let cipher = XChaCha20Poly1305::new(kek.expose().into());
    let opened = cipher
        .decrypt(
            nonce.into(),
            Payload {
                msg: wrapped,
                aad: header_prefix,
            },
        )
        .map_err(|_| CryptoError::Authentication)?;

    if opened.len() != KEY_LEN {
        return Err(CryptoError::Authentication);
    }
    let mut bytes = [0u8; KEY_LEN];
    bytes.copy_from_slice(&opened);
    Ok(MasterKey::from_bytes(bytes))
}
