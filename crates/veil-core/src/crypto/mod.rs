//! Key derivation, key hierarchy, streaming AEAD, and zeroisation (Spec §3).
//!
//! This module depends on no sibling module, so that splitting it into a
//! separate crate for independent audit stays a mechanical move (Spec §1).
//! Phase 0 to-do item P0.1.d enforces that with a check rather than with
//! intent; test case T0.4 is the check.
//!
//! Phase 0 defines the key types only. The hierarchy that fills them —
//! Argon2id, the wrapped master key, HKDF subkeys, and STREAM content
//! encryption — is Phase 1.

mod keys;

pub use keys::{Dek, EntryWrapKey, IndexKey, Kek, MasterKey, Password};
