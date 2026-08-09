//! Phase 5 test cases T5.1 through T5.4 — NFC name normalisation
//! (Spec §4.6; HC-8, FR-13, FR-10).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use harness::{SMALL_CAP, add, create, pattern};
use veil_core::{Cancel, NoProgress};

/// "café.txt", precomposed (`é` is one codepoint, U+00E9) — NFC.
const NFC_CAFE: &str = "caf\u{e9}.txt";
/// "café.txt", decomposed (`e` + a combining acute accent, U+0301) — NFD.
/// Visually identical to [`NFC_CAFE`] and byte-for-byte different.
const NFD_CAFE: &str = "cafe\u{301}.txt";

/// T5.1 — An NFD name and its NFC spelling store as one entry
/// (Spec §4.6, HC-8).
#[test]
fn t5_1_an_nfd_name_and_its_nfc_spelling_store_as_one_entry() {
    assert_ne!(
        NFC_CAFE.as_bytes(),
        NFD_CAFE.as_bytes(),
        "the two constants must actually differ in bytes, or this test proves nothing"
    );

    // Through `add`, supplying each spelling directly.
    let scratch = harness::Scratch::new("nfc-add-precomposed");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);
    let id = add(&mut vault, NFC_CAFE, "", &pattern(10));
    assert_eq!(
        vault.entries().iter().find(|e| e.id == id).unwrap().name,
        NFC_CAFE
    );

    let scratch = harness::Scratch::new("nfc-add-decomposed");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);
    let id = add(&mut vault, NFD_CAFE, "", &pattern(10));
    assert_eq!(
        vault.entries().iter().find(|e| e.id == id).unwrap().name,
        NFC_CAFE,
        "the NFD spelling must be stored as NFC, not as given"
    );

    // Through `add_path`, from a file whose real on-disk name is NFD.
    let scratch = harness::Scratch::new("nfc-add-path");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);
    let source = scratch.path(NFD_CAFE);
    std::fs::write(&source, pattern(10)).unwrap();
    let id = vault
        .add_path(&source, "", &mut NoProgress, &Cancel::new())
        .unwrap();
    assert_eq!(
        vault.entries().iter().find(|e| e.id == id).unwrap().name,
        NFC_CAFE,
        "a name read from the filesystem must be normalised too, not only a caller's literal"
    );
}

/// T5.2 — Matching a stored name by its other spelling
/// (Spec §4.6, FR-13).
#[test]
fn t5_2_matching_a_stored_name_by_its_other_spelling() {
    let scratch = harness::Scratch::new("nfc-match");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);
    let id = add(&mut vault, NFC_CAFE, "photos", &pattern(20));

    // `find` with the other spelling resolves to the same entry.
    let found = vault.find("photos", NFD_CAFE).unwrap();
    assert_eq!(found.id, id);

    // `replace` with the other spelling replaces the same entry, not a
    // second one.
    let replacement = pattern(30);
    let new_id = vault
        .replace(
            "photos",
            NFD_CAFE,
            &mut replacement.as_slice(),
            &mut NoProgress,
            &Cancel::new(),
        )
        .unwrap();
    assert_eq!(
        vault.entries().len(),
        1,
        "replace must not have inserted a second entry"
    );
    assert_eq!(harness::read_back(&vault, new_id).unwrap(), replacement);
}

/// T5.3 — Case sensitivity is unaffected by normalisation (Spec §4.6).
#[test]
fn t5_3_case_sensitivity_is_unaffected_by_normalisation() {
    let scratch = harness::Scratch::new("nfc-case");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);

    add(&mut vault, "Report.PDF", "d", &pattern(10));
    add(&mut vault, "report.pdf", "d", &pattern(20));

    assert_eq!(
        vault.entries().len(),
        2,
        "case must still distinguish two entries"
    );
    assert!(vault.find("d", "Report.PDF").is_some());
    assert!(vault.find("d", "report.pdf").is_some());
}

/// T5.4 — A folder walk over NFD-yielding paths produces NFC folder metadata
/// (Spec §4.6, FR-10).
#[test]
fn t5_4_a_folder_walk_over_nfd_yielding_paths_produces_nfc_folder_metadata() {
    let scratch = harness::Scratch::new("nfc-walk");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);

    let root = scratch.path("source");
    let folder = root.join(NFD_CAFE.trim_end_matches(".txt")); // an NFD *folder* name
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join(NFD_CAFE), pattern(15)).unwrap(); // and an NFD *file* name

    let outcome = vault
        .add_folder(&root, &mut NoProgress, &Cancel::new())
        .unwrap();
    assert_eq!(outcome.added.len(), 1);
    assert!(outcome.skipped.is_empty());

    let entry = vault.entries().first().unwrap();
    assert_eq!(entry.name, NFC_CAFE, "the walked file name must be NFC");
    assert_eq!(
        entry.folder,
        NFC_CAFE.trim_end_matches(".txt"),
        "the walked folder segment must be NFC too"
    );
}
