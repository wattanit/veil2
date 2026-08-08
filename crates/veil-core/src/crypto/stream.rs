//! Streaming content encryption (Spec §3.3, §4.7; HC-3, A-2, S-1, FR-17).
//!
//! `StreamBE32<XChaCha20Poly1305>` — the STREAM construction. **This is the
//! direct fix for the defect demonstrated in the original Veil**, where the
//! final chunk of a file could be removed and decryption still reported
//! success, exit code zero, and a short file. STREAM tags the last chunk
//! distinctly, so a stream that ends without one fails authentication.
//!
//! Three properties the loops below exist to hold:
//!
//! - **Nothing unauthenticated reaches the caller.** A chunk is written to the
//!   destination only after it has authenticated. Detection that arrives after
//!   bytes have been handed over is a report about data the user already
//!   holds, which is the shape of the original's defect rather than its fix.
//! - **Peak memory does not follow file size** (S-1, A-2). Two chunk buffers,
//!   reused, whatever the length of the input.
//! - **The hash is computed in the same pass** (FR-17, P1.6.a), so verifying
//!   an extraction end to end costs no second read.

use std::io::{Read, Write};

use aead_stream::{NewStream, StreamBE32, StreamPrimitive};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, aead::Payload};

use super::error::CryptoError;
use super::keys::{Dek, KEY_LEN};

/// Plaintext bytes per chunk.
///
/// Initial; tune with use. Balances per-chunk tag overhead — 16 bytes, which
/// is nothing at this size — against cancellation latency and memory (S-1).
pub const CHUNK_LEN: usize = 1024 * 1024;

/// Bytes the AEAD adds to each chunk.
pub const TAG_LEN: usize = 16;

/// Length of the per-entry nonce prefix.
///
/// STREAM consumes the remaining five bytes of the 192-bit nonce for its
/// counter and last-block flag. A fresh random prefix per entry, under a key
/// that is itself unique per entry, makes nonce reuse structurally impossible
/// rather than dependent on a counter being managed correctly.
pub const NONCE_PREFIX_LEN: usize = 19;

/// Length of a content hash.
pub const HASH_LEN: usize = 32;

/// What an encryption pass produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentSummary {
    /// Plaintext bytes consumed.
    pub plaintext_len: u64,
    /// Ciphertext bytes written.
    pub ciphertext_len: u64,
    /// BLAKE3 of the plaintext, computed in the same pass.
    pub hash: [u8; HASH_LEN],
}

/// Generates a fresh per-entry data key.
#[must_use]
pub fn generate_dek() -> Dek {
    let mut bytes = [0u8; KEY_LEN];
    getrandom::fill(&mut bytes).unwrap_or_else(|_| std::process::abort());
    Dek::from_bytes(bytes)
}

/// Generates a fresh per-entry nonce prefix.
#[must_use]
pub fn generate_nonce_prefix() -> [u8; NONCE_PREFIX_LEN] {
    let mut bytes = [0u8; NONCE_PREFIX_LEN];
    getrandom::fill(&mut bytes).unwrap_or_else(|_| std::process::abort());
    bytes
}

/// Bound to every chunk: the entry's identity, so a chunk cannot be
/// transplanted between entries (HC-3). STREAM already binds position.
fn associated_data(entry_id: u64) -> [u8; 8] {
    entry_id.to_le_bytes()
}

/// Reads up to `buf.len()` bytes, short only at end of input. Treating any
/// short read as EOF would split a chunk and produce ciphertext only this build
/// could read back.
fn read_fully(src: &mut impl Read, buf: &mut [u8]) -> Result<usize, CryptoError> {
    let mut filled = 0;
    while filled < buf.len() {
        match src.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err(CryptoError::Io),
        }
    }
    Ok(filled)
}

/// Called after each chunk with the cumulative plaintext byte count. Returning
/// an error stops the operation at that boundary — the one seam progress and
/// cancellation need (A-3). What stopping *means* belongs to the caller.
pub type ChunkHook<'a> = &'a mut dyn FnMut(u64) -> Result<(), CryptoError>;

/// Encrypts `src` into `dst`, hashing the plaintext as it goes.
///
/// # Errors
///
/// Fails on an I/O error or if the AEAD refuses a chunk.
pub fn encrypt(
    dek: &Dek,
    nonce_prefix: &[u8; NONCE_PREFIX_LEN],
    entry_id: u64,
    src: &mut impl Read,
    dst: &mut impl Write,
) -> Result<ContentSummary, CryptoError> {
    encrypt_watched(dek, nonce_prefix, entry_id, src, dst, &mut |_| Ok(()))
}

/// Encrypts as [`encrypt`] does, calling `on_chunk` at every chunk boundary.
///
/// Stopping costs up to one extra chunk of reading: knowing which chunk is last
/// requires reading the next one first.
///
/// # Errors
///
/// Fails on an I/O error, if the AEAD refuses a chunk, or with
/// [`CryptoError::Stopped`] when the hook asks to stop.
pub fn encrypt_watched(
    dek: &Dek,
    nonce_prefix: &[u8; NONCE_PREFIX_LEN],
    entry_id: u64,
    src: &mut impl Read,
    dst: &mut impl Write,
    on_chunk: ChunkHook<'_>,
) -> Result<ContentSummary, CryptoError> {
    let aad = associated_data(entry_id);
    let cipher = XChaCha20Poly1305::new(dek.expose().into());
    // `encrypt_last` consumes the encryptor, so it is held in an Option and
    // taken on the final chunk. Nothing follows a last chunk by construction.
    let mut stream = Some(StreamBE32::from_aead(cipher, nonce_prefix.into()).encryptor());

    let mut hasher = blake3::Hasher::new();
    let mut plaintext_len = 0u64;
    let mut ciphertext_len = 0u64;

    let mut current = vec![0u8; CHUNK_LEN];
    let mut lookahead = vec![0u8; CHUNK_LEN];

    let mut current_len = read_fully(src, &mut current)?;

    loop {
        // The final chunk is only known once the next read returns nothing.
        // STREAM tags the last chunk differently, and getting that wrong is
        // the defect this rebuild exists to fix.
        let next_len = if current_len < CHUNK_LEN {
            0
        } else {
            read_fully(src, &mut lookahead)?
        };

        hasher.update(&current[..current_len]);
        plaintext_len += current_len as u64;

        let payload = Payload {
            msg: &current[..current_len],
            aad: &aad,
        };

        if next_len == 0 {
            let Some(last) = stream.take() else {
                return Err(CryptoError::Authentication);
            };
            let sealed = last
                .encrypt_last(payload)
                .map_err(|_| CryptoError::Authentication)?;
            dst.write_all(&sealed).map_err(|_| CryptoError::Io)?;
            ciphertext_len += sealed.len() as u64;
            // Reported for the last chunk too: a single-chunk file reaches
            // the hook only here, and a limit that missed it is no limit.
            on_chunk(plaintext_len)?;
            break;
        }

        let Some(next) = stream.as_mut() else {
            return Err(CryptoError::Authentication);
        };
        let sealed = next
            .encrypt_next(payload)
            .map_err(|_| CryptoError::Authentication)?;
        dst.write_all(&sealed).map_err(|_| CryptoError::Io)?;
        ciphertext_len += sealed.len() as u64;
        on_chunk(plaintext_len)?;

        std::mem::swap(&mut current, &mut lookahead);
        current_len = next_len;
    }

    Ok(ContentSummary {
        plaintext_len,
        ciphertext_len,
        hash: *hasher.finalize().as_bytes(),
    })
}

/// Decrypts `src` into `dst`, verifying the content hash at the end.
///
/// Nothing is written to `dst` before the chunk it came from has
/// authenticated. When `expected_hash` is given, a mismatch fails the
/// operation even though every chunk authenticated — the second, independent
/// statement FR-17 requires, and the one that survives an attacker who can
/// rewrite an index but not forge under its key.
///
/// # Errors
///
/// [`CryptoError::Authentication`] on any alteration, truncation, reordering,
/// or substitution; [`CryptoError::ContentHashMismatch`] when the content
/// authenticates but is not what the index recorded.
pub fn decrypt(
    dek: &Dek,
    nonce_prefix: &[u8; NONCE_PREFIX_LEN],
    entry_id: u64,
    expected_hash: Option<&[u8; HASH_LEN]>,
    src: &mut impl Read,
    dst: &mut impl Write,
) -> Result<u64, CryptoError> {
    decrypt_watched(
        dek,
        nonce_prefix,
        entry_id,
        expected_hash,
        src,
        dst,
        &mut |_| Ok(()),
    )
}

/// Decrypts as [`decrypt`] does, calling `on_chunk` at every chunk boundary.
///
/// # Errors
///
/// As [`decrypt`], plus [`CryptoError::Stopped`] when the hook asks to stop.
#[allow(clippy::too_many_arguments)]
pub fn decrypt_watched(
    dek: &Dek,
    nonce_prefix: &[u8; NONCE_PREFIX_LEN],
    entry_id: u64,
    expected_hash: Option<&[u8; HASH_LEN]>,
    src: &mut impl Read,
    dst: &mut impl Write,
    on_chunk: ChunkHook<'_>,
) -> Result<u64, CryptoError> {
    let aad = associated_data(entry_id);
    let cipher = XChaCha20Poly1305::new(dek.expose().into());
    // `decrypt_last` consumes the decryptor, as above.
    let mut stream = Some(StreamBE32::from_aead(cipher, nonce_prefix.into()).decryptor());

    let sealed_len = CHUNK_LEN + TAG_LEN;
    let mut current = vec![0u8; sealed_len];
    let mut lookahead = vec![0u8; sealed_len];

    let mut hasher = blake3::Hasher::new();
    let mut plaintext_len = 0u64;

    let mut current_len = read_fully(src, &mut current)?;
    if current_len == 0 {
        // Not even a tag. Every entry has a final chunk, so an empty stream
        // is truncation, not an empty file.
        return Err(CryptoError::Authentication);
    }

    loop {
        let next_len = if current_len < sealed_len {
            0
        } else {
            read_fully(src, &mut lookahead)?
        };

        let payload = Payload {
            msg: &current[..current_len],
            aad: &aad,
        };

        // Truncation fails here: a removed final chunk turns something
        // encrypted with `encrypt_next` into a `decrypt_last`, and the tags
        // disagree.
        if next_len == 0 {
            let Some(last) = stream.take() else {
                return Err(CryptoError::Authentication);
            };
            let opened = last
                .decrypt_last(payload)
                .map_err(|_| CryptoError::Authentication)?;
            // Only now, after authentication.
            hasher.update(&opened);
            plaintext_len += opened.len() as u64;
            dst.write_all(&opened).map_err(|_| CryptoError::Io)?;
            on_chunk(plaintext_len)?;
            break;
        }

        let Some(next) = stream.as_mut() else {
            return Err(CryptoError::Authentication);
        };
        let opened = next
            .decrypt_next(payload)
            .map_err(|_| CryptoError::Authentication)?;
        hasher.update(&opened);
        plaintext_len += opened.len() as u64;
        dst.write_all(&opened).map_err(|_| CryptoError::Io)?;
        on_chunk(plaintext_len)?;

        std::mem::swap(&mut current, &mut lookahead);
        current_len = next_len;
    }

    if let Some(expected) = expected_hash
        && hasher.finalize().as_bytes() != expected
    {
        return Err(CryptoError::ContentHashMismatch);
    }

    Ok(plaintext_len)
}
