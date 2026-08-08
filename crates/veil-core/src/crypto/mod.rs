//! Key derivation, key hierarchy, streaming AEAD, and zeroisation (Spec §3).
//!
//! Depends on no sibling module, so splitting it into its own crate for audit
//! stays a mechanical move. It carries its own error type for the same reason;
//! the conversion into the crate taxonomy lives in `error`. Test T0.4 enforces
//! this rather than trusting intent.

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
    CHUNK_LEN, ChunkHook, ContentSummary, HASH_LEN, NONCE_PREFIX_LEN, TAG_LEN, decrypt,
    decrypt_watched, encrypt, encrypt_watched, generate_dek, generate_nonce_prefix,
};
pub use subkeys::{entry_wrap_key, index_key};
pub use wrap::{
    WRAP_NONCE_LEN, WRAPPED_KEY_LEN, generate_master_key, unwrap_dek, unwrap_master_key, wrap_dek,
    wrap_master_key,
};
