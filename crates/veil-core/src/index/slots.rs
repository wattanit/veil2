//! Double-buffered atomic index persistence (Spec §4.4; HC-4, FR-27).
//!
//! Two slots. A write goes to the slot holding the *older* generation and
//! fsyncs; a read takes the highest generation that authenticates. No rename —
//! rename atomicity varies by platform while "the older slot is expendable"
//! holds everywhere, so a crash mid-write only damages the expendable one.
//!
//! The generation appears in the plaintext preamble, so slots can be ordered
//! without decrypting, and inside the document. The preamble is bound as
//! associated data, so a stale slot cannot be made to look current.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305};

use crate::crypto::IndexKey;
use crate::error::{Damaged, Error, Result};

use super::document::IndexDocument;

/// Identifies a slot file.
const SLOT_MAGIC: [u8; 4] = *b"VIX1";

const NONCE_LEN: usize = 24;
const PREAMBLE_LEN: usize = SLOT_MAGIC.len() + 8 + NONCE_LEN;

/// The two slot file names (Spec §4.1).
const SLOT_NAMES: [&str; 2] = ["index.a", "index.b"];

/// One slot's readable state.
struct SlotState {
    path: PathBuf,
    /// `None` when the slot is absent or unreadable as a slot at all.
    generation: Option<u64>,
}

fn slot_paths(vault_dir: &Path) -> [PathBuf; 2] {
    [vault_dir.join(SLOT_NAMES[0]), vault_dir.join(SLOT_NAMES[1])]
}

/// Reads a slot's plaintext generation without decrypting it.
fn peek(path: &Path) -> SlotState {
    let generation = fs::read(path).ok().and_then(|bytes| {
        if bytes.len() < PREAMBLE_LEN || bytes[..4] != SLOT_MAGIC {
            return None;
        }
        let mut raw = [0u8; 8];
        raw.copy_from_slice(&bytes[4..12]);
        Some(u64::from_le_bytes(raw))
    });
    SlotState {
        path: path.to_path_buf(),
        generation,
    }
}

fn decrypt_slot(path: &Path, key: &IndexKey) -> Option<IndexDocument> {
    let bytes = fs::read(path).ok()?;
    if bytes.len() < PREAMBLE_LEN || bytes[..4] != SLOT_MAGIC {
        return None;
    }
    let preamble = &bytes[..PREAMBLE_LEN];
    let nonce: [u8; NONCE_LEN] = bytes[12..PREAMBLE_LEN].try_into().ok()?;

    let cipher = XChaCha20Poly1305::new(key.expose().into());
    let opened = cipher
        .decrypt(
            (&nonce).into(),
            Payload {
                msg: &bytes[PREAMBLE_LEN..],
                aad: preamble,
            },
        )
        .ok()?;

    let document = IndexDocument::from_cbor(&opened).ok()?;

    // The preamble is authenticated, so it cannot have been altered — but a
    // writer bug could still have written two different numbers. A slot whose
    // two generations disagree is not trustworthy.
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&bytes[4..12]);
    if u64::from_le_bytes(raw) != document.generation {
        return None;
    }
    Some(document)
}

/// Reads the index, taking the highest generation that authenticates, falling
/// back to the other slot rather than failing (HC-4).
///
/// # Errors
///
/// [`Error::Corrupt`] with [`Damaged::BothIndexSlots`] when neither slot can
/// be read. Never an empty index, never a partial one, never a guess.
pub fn read(vault_dir: &Path, key: &IndexKey) -> Result<IndexDocument> {
    let paths = slot_paths(vault_dir);
    let mut states: Vec<SlotState> = paths.iter().map(|p| peek(p)).collect();

    // Highest generation first; a slot we could not even peek at goes last.
    states.sort_by_key(|s| std::cmp::Reverse(s.generation));

    for state in &states {
        if state.generation.is_none() {
            continue;
        }
        if let Some(document) = decrypt_slot(&state.path, key) {
            return Ok(document);
        }
    }

    Err(Error::Corrupt {
        what: Damaged::BothIndexSlots,
        affected: Vec::new(),
    })
}

/// Writes the index to the slot holding the older generation, and fsyncs.
/// Does not touch `document.generation` — one generation per committed mutation
/// is the caller's transaction to keep, not the writer's.
///
/// # Errors
///
/// [`Error::Io`] if the write or the fsync fails.
pub fn write(vault_dir: &Path, key: &IndexKey, document: &IndexDocument) -> Result<()> {
    let paths = slot_paths(vault_dir);
    let states: Vec<SlotState> = paths.iter().map(|p| peek(p)).collect();

    // The expendable slot: absent, unreadable, or older. Writing over the
    // current one would risk both generations in a single crash.
    let target = if states[0].generation.is_none() {
        &paths[0]
    } else if states[1].generation.is_none() {
        &paths[1]
    } else if states[0].generation <= states[1].generation {
        &paths[0]
    } else {
        &paths[1]
    };

    let plaintext = document.to_cbor().map_err(|_| Error::Corrupt {
        what: Damaged::IndexSlot,
        affected: Vec::new(),
    })?;

    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).map_err(|_| Error::Io {
        kind: std::io::ErrorKind::Other,
    })?;

    let mut preamble = Vec::with_capacity(PREAMBLE_LEN);
    preamble.extend_from_slice(&SLOT_MAGIC);
    preamble.extend_from_slice(&document.generation.to_le_bytes());
    preamble.extend_from_slice(&nonce);

    let cipher = XChaCha20Poly1305::new(key.expose().into());
    let sealed = cipher
        .encrypt(
            (&nonce).into(),
            Payload {
                msg: &plaintext,
                aad: &preamble,
            },
        )
        .map_err(|_| Error::Corrupt {
            what: Damaged::IndexSlot,
            affected: Vec::new(),
        })?;

    // A slot written for the first time is a new name in the vault directory,
    // and a name is durable only once the directory is (§4.7, HC-4).
    let named_a_slot = !target.exists();

    let mut file = fs::File::create(target)?;
    file.write_all(&preamble)?;
    file.write_all(&sealed)?;
    // Success is only reportable once the bytes are durable (FR-12, HC-4).
    file.sync_all()?;
    if named_a_slot {
        crate::durable::sync_dir(vault_dir)?;
    }
    Ok(())
}

/// The generations currently on disk, for tests and diagnostics.
#[must_use]
pub fn generations(vault_dir: &Path) -> [Option<u64>; 2] {
    let paths = slot_paths(vault_dir);
    [peek(&paths[0]).generation, peek(&paths[1]).generation]
}
