//! Phase 1 test cases T1.10 through T1.19 — content encryption and the
//! adversarial corruption suite (HC-3, A-2, S-1, FR-18, Spec §3.3, §9).
//!
//! **Every mutation is applied to the ciphertext bytes**, not through the API.
//! The attacker's position is the file; a suite that mutates through the API
//! tests the API's tolerance of its own values and proves nothing about a
//! vault that arrived from a stolen drive.
//!
//! Every case asserts *which* failure, not merely that one occurred. "Returns
//! an error" is satisfied by a build that fails on everything.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use veil_core::crypto::{
    CHUNK_LEN, CryptoError, Dek, HASH_LEN, NONCE_PREFIX_LEN, TAG_LEN, decrypt, encrypt,
    generate_dek, generate_nonce_prefix,
};

const ENTRY: u64 = 42;
const SEALED_CHUNK: usize = CHUNK_LEN + TAG_LEN;

struct Fixture {
    dek: Dek,
    nonce: [u8; NONCE_PREFIX_LEN],
    plaintext: Vec<u8>,
    ciphertext: Vec<u8>,
    hash: [u8; HASH_LEN],
}

fn pattern(len: usize) -> Vec<u8> {
    // Not random: a repeating pattern makes a wrongly-ordered or transplanted
    // chunk visible in a failure message.
    (0..len).map(|i| (i % 251) as u8).collect()
}

fn seal(plaintext: Vec<u8>) -> Fixture {
    let dek = generate_dek();
    let nonce = generate_nonce_prefix();
    let mut ciphertext = Vec::new();
    let summary = encrypt(
        &dek,
        &nonce,
        ENTRY,
        &mut plaintext.as_slice(),
        &mut ciphertext,
    )
    .expect("encrypt");
    assert_eq!(summary.plaintext_len, plaintext.len() as u64);
    assert_eq!(summary.ciphertext_len, ciphertext.len() as u64);
    Fixture {
        dek,
        nonce,
        plaintext,
        ciphertext,
        hash: summary.hash,
    }
}

impl Fixture {
    /// Decrypts the given bytes, discarding output.
    fn open(&self, bytes: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let mut out = Vec::new();
        decrypt(
            &self.dek,
            &self.nonce,
            ENTRY,
            Some(&self.hash),
            &mut &bytes[..],
            &mut out,
        )?;
        Ok(out)
    }

    /// Decrypts without the hash check, isolating chunk authentication.
    fn open_unhashed(&self, bytes: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let mut out = Vec::new();
        decrypt(
            &self.dek,
            &self.nonce,
            ENTRY,
            None,
            &mut &bytes[..],
            &mut out,
        )?;
        Ok(out)
    }
}

/// The sizes that exercise every boundary in the chunking loop.
fn sizes() -> Vec<(&'static str, usize)> {
    vec![
        ("empty", 0),
        ("one byte", 1),
        ("one under a chunk", CHUNK_LEN - 1),
        ("exactly one chunk", CHUNK_LEN),
        ("one over a chunk", CHUNK_LEN + 1),
        ("two whole chunks", CHUNK_LEN * 2),
        ("two chunks and a tail", CHUNK_LEN * 2 + 7),
    ]
}

/// T1.10 — content round-trips byte-identically (HC-3, A-2, Spec §3.3).
#[test]
fn t1_10_round_trip_is_byte_identical() {
    for (label, len) in sizes() {
        let f = seal(pattern(len));
        let out = f
            .open(&f.ciphertext)
            .unwrap_or_else(|e| panic!("{label}: {e}"));
        assert_eq!(out, f.plaintext, "{label}: content changed");

        // The hash is computed in the same pass as encryption (P1.6.a); it
        // must match one computed independently of the write path.
        assert_eq!(
            f.hash,
            *blake3::hash(&f.plaintext).as_bytes(),
            "{label}: recorded hash is not BLAKE3 of the plaintext"
        );
    }
}

/// T1.12 — a single flipped byte fails authentication (HC-3) — §9 row 1.
#[test]
fn t1_12_single_bit_flip_fails() {
    let f = seal(pattern(CHUNK_LEN * 2 + 7));
    let positions = [
        ("first chunk body", 0usize),
        ("first chunk tag", SEALED_CHUNK - 1),
        ("middle chunk body", SEALED_CHUNK + 10),
        ("final chunk body", f.ciphertext.len() - 20),
        ("final chunk tag", f.ciphertext.len() - 1),
    ];

    for (label, at) in positions {
        let mut damaged = f.ciphertext.clone();
        damaged[at] ^= 0x01;
        match f.open(&damaged) {
            Err(CryptoError::Authentication) => {}
            other => panic!("{label}: expected authentication failure, got {other:?}"),
        }
    }
}

/// T1.13 — a truncated final chunk fails (HC-3) — §9 row 2.
///
/// **This is the direct regression test for the defect that ended the
/// original.** Removing the final chunk of a three-megabyte file there
/// produced a two-megabyte file, a success message, and exit code zero — and
/// because extraction wrote into the working directory, it overwrote the
/// user's own original with the truncated result.
///
/// HC-3's clause that partial output is never reported as success exists
/// because of this case. If only one test in this file ever runs, it is this
/// one.
#[test]
fn t1_13_truncated_final_chunk_fails() {
    let f = seal(pattern(CHUNK_LEN * 3));
    assert!(f.ciphertext.len() > SEALED_CHUNK * 2);

    // Remove the whole final chunk, leaving a stream of well-formed,
    // individually authentic chunks that simply stops early. Every byte that
    // remains is genuine, which is exactly why this must still fail.
    let truncated = &f.ciphertext[..SEALED_CHUNK * 2];
    match f.open(truncated) {
        Err(CryptoError::Authentication) => {}
        other => panic!("a truncated file decrypted: {other:?}"),
    }
}

/// T1.14 — truncation within a chunk fails (HC-3) — §9 row 3.
#[test]
fn t1_14_truncation_within_a_chunk_fails() {
    let f = seal(pattern(CHUNK_LEN + 5000));
    for cut in [1usize, 17, 999] {
        let truncated = &f.ciphertext[..f.ciphertext.len() - cut];
        match f.open(truncated) {
            Err(CryptoError::Authentication) => {}
            other => panic!("removing {cut} trailing bytes decrypted: {other:?}"),
        }
    }
}

/// T1.15 — reordered chunks fail (HC-3) — §9 row 4.
///
/// Every individual chunk here is authentic. Only their order changed, and
/// STREAM binds position, so this must still fail.
#[test]
fn t1_15_reordered_chunks_fail() {
    let f = seal(pattern(CHUNK_LEN * 3));
    let mut swapped = f.ciphertext.clone();
    let (first, rest) = swapped.split_at_mut(SEALED_CHUNK);
    first.swap_with_slice(&mut rest[..SEALED_CHUNK]);

    match f.open(&swapped) {
        Err(CryptoError::Authentication) => {}
        other => panic!("reordered chunks decrypted: {other:?}"),
    }
}

/// T1.16 — a chunk cannot be transplanted between entries (HC-3, §3.3) —
/// §9 row 5.
///
/// Two sub-cases, and both are needed. Per-entry keys make the integration
/// case fail on the key alone, so on its own it would pass against a build
/// that never binds entry identity as associated data at all. The second
/// sub-case is the one that actually tests the Specification's claim.
#[test]
fn t1_16_chunks_cannot_be_transplanted() {
    // Integration: two entries, two keys.
    let a = seal(pattern(CHUNK_LEN * 2));
    let b = seal(pattern(CHUNK_LEN * 2));
    let mut spliced = a.ciphertext.clone();
    spliced[..SEALED_CHUNK].copy_from_slice(&b.ciphertext[..SEALED_CHUNK]);
    match a.open(&spliced) {
        Err(CryptoError::Authentication) => {}
        other => panic!("a transplanted chunk decrypted: {other:?}"),
    }

    // Construction: one key, one nonce prefix, two entry identities. This is
    // the case that fails if the entry id stops being associated data.
    let dek = generate_dek();
    let nonce = generate_nonce_prefix();
    let plaintext = pattern(4096);
    let mut sealed = Vec::new();
    encrypt(&dek, &nonce, 1, &mut plaintext.as_slice(), &mut sealed).unwrap();

    let mut out = Vec::new();
    match decrypt(&dek, &nonce, 2, None, &mut &sealed[..], &mut out) {
        Err(CryptoError::Authentication) => {}
        other => panic!("content decrypted under a different entry id: {other:?}"),
    }
    assert!(
        out.is_empty(),
        "plaintext was written despite the failure (HC-3)"
    );

    // And under its own identity it opens, so the check above is not passing
    // merely because nothing decrypts.
    let mut good = Vec::new();
    decrypt(&dek, &nonce, 1, None, &mut &sealed[..], &mut good).unwrap();
    assert_eq!(good, plaintext);
}

/// T1.17 — extending an entry's stored bytes fails (HC-3).
///
/// *Trace note:* this row is not in the Specification's §9 table. It is the
/// counterpart of truncation, covered by HC-3's "any alteration", and is
/// recorded as an addition so the correspondence between the suite and §9
/// stays legible (P1.11.d).
#[test]
fn t1_17_appending_to_the_stream_fails() {
    let f = seal(pattern(CHUNK_LEN + 100));

    // Arbitrary trailing bytes.
    let mut extended = f.ciphertext.clone();
    extended.extend_from_slice(&[0u8; 64]);
    assert!(
        matches!(f.open(&extended), Err(CryptoError::Authentication)),
        "arbitrary trailing bytes were accepted"
    );

    // A well-formed chunk from the same entry, appended after its last chunk.
    let mut duplicated = f.ciphertext.clone();
    duplicated.extend_from_slice(&f.ciphertext[..SEALED_CHUNK]);
    assert!(
        matches!(f.open(&duplicated), Err(CryptoError::Authentication)),
        "a duplicated chunk was accepted after the final chunk"
    );
}

/// T1.18 — a content-hash mismatch fails a read that otherwise authenticated
/// (FR-18, HC-3).
///
/// Chunk authentication proves each chunk is what was written under this
/// entry's key. It does not prove the recorded hash was not swapped in the
/// index. FR-18 is the second, independent statement, and this is the only
/// case that exercises it in isolation.
#[test]
fn t1_18_content_hash_mismatch_fails() {
    let f = seal(pattern(CHUNK_LEN + 11));

    // Every chunk authenticates.
    f.open_unhashed(&f.ciphertext)
        .expect("chunks authenticate on their own");

    // With a hash that is not this content's, the read still fails.
    let mut wrong = f.hash;
    wrong[0] ^= 0xFF;
    let mut out = Vec::new();
    match decrypt(
        &f.dek,
        &f.nonce,
        ENTRY,
        Some(&wrong),
        &mut &f.ciphertext[..],
        &mut out,
    ) {
        Err(CryptoError::ContentHashMismatch) => {}
        other => panic!("expected a hash mismatch, got {other:?}"),
    }
}

/// T1.19 — no unauthenticated plaintext reaches the caller (HC-3).
///
/// Detection that arrives after bytes have been handed over is a report about
/// data the user already holds — the shape of the original's defect rather
/// than its fix. The sink below records everything it is given, so anything
/// released early is visible here.
#[test]
fn t1_19_failing_chunks_release_no_plaintext() {
    let f = seal(pattern(CHUNK_LEN * 2));

    // Damage the *final* chunk: the first chunk is genuine, so a build that
    // streamed output as it went would have written a megabyte before noticing.
    let mut damaged = f.ciphertext.clone();
    let last = damaged.len() - 1;
    damaged[last] ^= 0xFF;

    let mut sink = Vec::new();
    let result = decrypt(
        &f.dek,
        &f.nonce,
        ENTRY,
        Some(&f.hash),
        &mut &damaged[..],
        &mut sink,
    );
    assert!(matches!(result, Err(CryptoError::Authentication)));

    // The first chunk was authentic, so it is legitimately written before the
    // failure is reachable — but nothing from the failing chunk may appear.
    assert_eq!(
        sink.len(),
        CHUNK_LEN,
        "output does not stop at the last chunk that authenticated"
    );
    assert_eq!(
        sink[..],
        f.plaintext[..CHUNK_LEN],
        "released bytes are not the ones that authenticated"
    );
}

/// T1.19 — a stream with no final chunk at all fails (HC-3).
#[test]
fn t1_19_empty_stream_is_truncation_not_an_empty_file() {
    let f = seal(Vec::new());
    // An empty file still produces a final chunk carrying its tag.
    assert_eq!(f.ciphertext.len(), TAG_LEN);
    assert!(f.open(&f.ciphertext).unwrap().is_empty());

    // Nothing at all is a truncation.
    assert!(matches!(f.open(&[]), Err(CryptoError::Authentication)));
}
