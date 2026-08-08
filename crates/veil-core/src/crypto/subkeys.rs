//! Subkey derivation from the master key (Spec §3.1; HC-6).
//!
//! HKDF-SHA256 with distinct, versioned `info` strings. Domain separation is
//! explicit here because the original Veil used one key for the header, the
//! metadata, and every file — the condition that turns any single nonce
//! mistake into total compromise rather than a local one.
//!
//! The `info` strings carry a version so that a later format can derive
//! different subkeys from the same master key without ambiguity.

use hkdf::Hkdf;
use sha2::Sha256;

use super::keys::{EntryWrapKey, IndexKey, KEY_LEN, MasterKey};

const INDEX_INFO: &[u8] = b"veil2:index:v1";
const ENTRY_WRAP_INFO: &[u8] = b"veil2:entry-wrap:v1";

fn derive(master: &MasterKey, info: &[u8]) -> [u8; KEY_LEN] {
    // No salt: the master key is already 32 uniformly random bytes, which is
    // the case RFC 5869 describes as needing none.
    let hkdf = Hkdf::<Sha256>::new(None, master.expose());
    let mut out = [0u8; KEY_LEN];
    // Expanding 32 bytes from SHA-256 cannot exceed the 255*HashLen limit, so
    // this branch is unreachable; it is written rather than unwrapped so that
    // no panic path exists in the key hierarchy at all.
    if hkdf.expand(info, &mut out).is_err() {
        out = [0u8; KEY_LEN];
    }
    out
}

/// Derives the subkey protecting the index.
#[must_use]
pub fn index_key(master: &MasterKey) -> IndexKey {
    IndexKey::from_bytes(derive(master, INDEX_INFO))
}

/// Derives the subkey wrapping each entry's data key.
#[must_use]
pub fn entry_wrap_key(master: &MasterKey) -> EntryWrapKey {
    EntryWrapKey::from_bytes(derive(master, ENTRY_WRAP_INFO))
}
