//! Phase 2 test cases T2.16, T2.17, and T2.19 — extraction
//! (FR-16, FR-17, HC-2, HC-3, S-4, Spec §4.7).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use harness::{SMALL_CAP, add, create, pattern};
use veil_core::crypto::CHUNK_LEN;
use veil_core::{Cancel, Damaged, Error, NoProgress};

/// T2.16 — content survives a round trip through the public API
/// (FR-16, FR-17).
///
/// Phase 1's T1.10 proved this for the stream. This proves it for the API a
/// frontend actually calls, across the extent and pack machinery between them —
/// including the lengths that sit on a chunk boundary, where a lookahead
/// implementation breaks.
#[test]
fn t2_16_content_survives_a_round_trip_through_the_public_api() {
    let scratch = harness::Scratch::new("round-trip");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);

    let lengths = [
        0,
        1,
        SMALL_CAP as usize - 1,
        CHUNK_LEN - 1,
        CHUNK_LEN,
        CHUNK_LEN + 1,
        CHUNK_LEN * 2 + 13,
    ];

    for (index, length) in lengths.into_iter().enumerate() {
        let content = pattern(length);
        let id = add(&mut vault, &format!("f{index}.bin"), "d", &content);
        assert_eq!(
            harness::read_back(&vault, id).unwrap(),
            content,
            "length {length} did not round trip"
        );
    }
}

/// T2.17 — a damaged entry produces no output file (FR-17, HC-3, S-4).
///
/// The original Veil left truncated plaintext in place and exited zero.
///
/// **The hash-comparison branch is not reachable from here, deliberately.**
/// Making the recorded hash disagree with intact content means rewriting the
/// index, which needs the index key — and an attacker who has it has the vault.
/// That branch is exercised where it can be reached, at the layer that owns it
/// (T1.18, T1.19). What this case owns is the API-level obligation: on failure,
/// the entry is named and no output file is left behind.
#[test]
fn t2_17_a_damaged_entry_produces_no_output_file() {
    let scratch = harness::Scratch::new("damaged-extract");
    let dir = scratch.vault_dir();

    let mut vault = create(&dir, SMALL_CAP);
    let content = pattern(9000);
    let target = add(&mut vault, "target.bin", "d", &content);
    let bystander = add(&mut vault, "bystander.bin", "d", &pattern(500));
    drop(vault);

    // Flip a byte inside the first pack the target occupies.
    let extent = {
        let vault = harness::open(&dir).unwrap();
        vault
            .entries()
            .iter()
            .find(|e| e.id == target)
            .unwrap()
            .extents[0]
    };
    harness::flip_byte_in_pack(&dir, extent.pack_id, extent.offset + 8);

    let vault = harness::open(&dir).unwrap();
    let destination = scratch.path("out.bin");
    let outcome = vault.extract_to_path(target, &destination, &mut NoProgress, &Cancel::new());

    match outcome {
        Err(Error::Corrupt { what, affected }) => {
            assert_eq!(what, Damaged::Content);
            // Named, because S-4 requires a partial failure to be presented as
            // a list of unreadable files rather than a failure of the vault.
            assert_eq!(affected, vec![target]);
        }
        other => panic!("expected damaged content naming the entry, got {other:?}"),
    }

    assert!(
        !destination.exists(),
        "a partial extraction was left looking like a valid file"
    );

    // The bystander is unaffected: damage costs only what it touches.
    assert_eq!(harness::read_back(&vault, bystander).unwrap(), pattern(500));
}

/// T2.19 — extraction writes only where the caller said (HC-2, FR-16).
///
/// **A direct regression test.** The original Veil chose the destination
/// itself, wrote into the working directory, and overwrote the user's original.
/// `extract` takes a `Write` and never learns a path, which is what makes this
/// structural rather than a discipline.
#[test]
fn t2_19_extraction_writes_only_where_the_caller_said() {
    let scratch = harness::Scratch::new("containment");
    let dir = scratch.vault_dir();

    // A file in the enclosing directory with the same name as the entry — the
    // exact collision the original destroyed.
    let bystander = scratch.path("original.bin");
    let bystander_content = b"THE USER'S OWN FILE".to_vec();
    std::fs::write(&bystander, &bystander_content).unwrap();

    let before = harness::snapshot(&scratch.0);

    let mut vault = create(&dir, SMALL_CAP);
    let id = add(&mut vault, "original.bin", "d", &pattern(9000));

    let mut out = Vec::new();
    vault
        .extract(id, &mut out, &mut NoProgress, &Cancel::new())
        .unwrap();
    assert_eq!(out, pattern(9000));
    drop(vault);

    assert_eq!(
        std::fs::read(&bystander).unwrap(),
        bystander_content,
        "extraction overwrote a file outside the vault"
    );

    let after = harness::snapshot(&scratch.0);
    let escaped: Vec<&String> = after
        .keys()
        .filter(|name| !before.contains_key(*name))
        .filter(|name| !name.starts_with("Test.veil/"))
        .collect();
    assert!(
        escaped.is_empty(),
        "files appeared outside the vault directory: {escaped:?}"
    );
}
