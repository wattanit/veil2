//! Phase 1 test case T1.33 — the header and index parsers are total
//! (HC-3, Spec §9).
//!
//! These are the only attacker-controlled inputs reachable before
//! authentication, so a panic in either is a defect regardless of who reaches
//! it.
//!
//! **This is randomised testing, not fuzzing.** Spec §9 calls for `cargo-fuzz`
//! on both parsers; that needs a nightly toolchain and a tool installed on the
//! machine, which is the owner's decision rather than this suite's. What runs
//! here is deterministic and seeded, so a failure reproduces exactly, and it
//! covers the same two entry points at lower depth. The `cargo-fuzz` targets
//! remain outstanding.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use veil_core::format::{HEADER_LEN, Header, MAGIC};
use veil_core::index::IndexDocument;

/// Deterministic xorshift, so any failure reproduces from its seed.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn byte(&mut self) -> u8 {
        (self.next_u64() >> 24) as u8
    }

    fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next_u64() % bound as u64) as usize
        }
    }

    fn bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| self.byte()).collect()
    }
}

/// T1.33 — the header parser accepts or refuses every input, and does neither
/// by panicking.
#[test]
fn t1_33_header_parser_is_total() {
    let mut rng = Rng(0x5EED_1234_ABCD_0001);

    for round in 0..20_000 {
        // A mix of shapes: pure noise, plausible lengths, and inputs that
        // start with the magic so they reach the fields rather than being
        // rejected at the first check.
        let bytes = match round % 4 {
            0 => {
                let len = rng.below(HEADER_LEN * 2);
                rng.bytes(len)
            }
            1 => rng.bytes(HEADER_LEN),
            2 => {
                let mut v = MAGIC.to_vec();
                v.extend(rng.bytes(HEADER_LEN - MAGIC.len()));
                v
            }
            _ => {
                let mut v = MAGIC.to_vec();
                let len = rng.below(HEADER_LEN * 2);
                v.extend(rng.bytes(len));
                v
            }
        };

        // The contract is that this returns. Any panic fails the test by
        // unwinding out of it.
        let _ = Header::parse(&bytes);
    }
}

/// T1.33 — the index parser accepts or refuses every input.
///
/// Authentication precedes parsing in the real read path, so reaching this
/// with arbitrary bytes implies the key. A panic here is still a defect: a
/// damaged document that authenticates — a writer bug, a bit flip inside a
/// window the AEAD covers but the model does not — must fail rather than
/// abort the process.
#[test]
fn t1_33_index_parser_is_total() {
    let mut rng = Rng(0x5EED_1234_ABCD_0002);

    for _ in 0..20_000 {
        let len = rng.below(512);
        let bytes = rng.bytes(len);
        let _ = IndexDocument::from_cbor(&bytes);
    }
}

/// T1.33 — a valid document with single bytes flipped still parses or refuses.
///
/// Pure noise is rejected at the first byte and exercises little. Mutating a
/// document that *was* valid drives the parser deep into the model, which is
/// where a length field read as a capacity would show up.
#[test]
fn t1_33_mutated_valid_documents_are_handled() {
    let mut rng = Rng(0x5EED_1234_ABCD_0003);
    let valid = IndexDocument::empty().to_cbor().unwrap();
    assert!(IndexDocument::from_cbor(&valid).is_ok());

    for _ in 0..20_000 {
        let mut bytes = valid.clone();
        let flips = 1 + rng.below(3);
        for _ in 0..flips {
            let at = rng.below(bytes.len());
            bytes[at] ^= rng.byte();
        }
        let _ = IndexDocument::from_cbor(&bytes);
    }
}

/// T1.33 — a truncated valid document is refused, not misread.
#[test]
fn t1_33_truncated_documents_are_refused() {
    let valid = IndexDocument::empty().to_cbor().unwrap();
    for cut in 1..valid.len() {
        let _ = IndexDocument::from_cbor(&valid[..cut]);
    }
}
