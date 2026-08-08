//! Turning a header and a password into a master key (FR-2, HC-3, §3.1).
//!
//! **This is the one place a wrong password is told apart from a damaged
//! vault.** The AEAD cannot make that distinction: the whole header is its
//! associated data, so a typo and an altered salt both surface as one
//! authentication failure. Classifying it anywhere else would mean two call
//! sites eventually disagreeing, which is how the original Veil ended up
//! reporting every failure as a cryptography error and sending users with a
//! typo to look for a damaged file.
//!
//! The rule: a header whose own checksum matches and whose fields are in range
//! is well formed, so an unwrap failure against it is a wrong password. A
//! header that fails those checks is damaged, and `Header::parse` has already
//! said so before this function runs.

use crate::crypto::{MasterKey, Password, derive_kek, unwrap_master_key};
use crate::error::{Error, Result};

use super::header::{HEADER_LEN, Header};

/// Parses a header and unwraps its master key.
///
/// # Errors
///
/// [`Error::NotAVault`], [`Error::FormatTooNew`], [`Error::FormatSuperseded`],
/// [`Error::Corrupt`] for a damaged header, or [`Error::WrongPassword`].
pub fn unlock(bytes: &[u8], password: &Password) -> Result<(Header, MasterKey)> {
    let header = Header::parse(bytes)?;

    let kek = derive_kek(
        header.kdf_algorithm,
        header.kdf_params,
        &header.kdf_salt,
        password,
    )?;

    let mut fixed = [0u8; HEADER_LEN];
    fixed.copy_from_slice(&bytes[..HEADER_LEN]);
    let prefix = Header::prefix(&fixed);

    let master = unwrap_master_key(&kek, &header.wrap_nonce, prefix, &header.wrapped_master_key)
        .map_err(|_| Error::WrongPassword)?;

    Ok((header, master))
}
