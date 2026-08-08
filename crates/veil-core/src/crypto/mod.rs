//! Key derivation, key hierarchy, streaming AEAD, and zeroisation (Spec §3).
//!
//! This module depends on no sibling module, so that splitting it into a
//! separate crate for independent audit stays a mechanical move (Spec §1).
//! It carries its own error type for the same reason; the conversion into the
//! crate taxonomy lives in `error`, pointing the dependency the other way.
//! Phase 0 to-do item P0.1.d enforces the rule with a check rather than with
//! intent, and test case T0.4 is the check.

mod error;
mod kdf;
mod keys;
mod stream;
mod subkeys;
mod wrap;

pub use error::CryptoError;
pub use kdf::{KdfAlgorithm, KdfParams, derive_kek};
pub use keys::{Dek, EntryWrapKey, IndexKey, Kek, MasterKey, Password};
pub use stream::{
    CHUNK_LEN, ContentSummary, HASH_LEN, NONCE_PREFIX_LEN, TAG_LEN, decrypt, encrypt, generate_dek,
    generate_nonce_prefix,
};
pub use subkeys::{entry_wrap_key, index_key};
pub use wrap::{
    WRAP_NONCE_LEN, WRAPPED_KEY_LEN, generate_master_key, unwrap_master_key, wrap_master_key,
};
