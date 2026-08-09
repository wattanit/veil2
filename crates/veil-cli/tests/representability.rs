//! Phase 5 test cases T5.9 and T5.10 — extraction representability at the
//! CLI (Spec §4.6, §5.2; FR-31).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use harness::Scratch;

/// T5.9 — `save-copy` surfaces the refusal without touching the destination
/// (FR-31, Spec §5.2).
#[test]
fn t5_9_save_copy_refuses_into_a_folder_without_touching_it() {
    let scratch = Scratch::new("representable-into-folder");
    let vault = scratch.vault_arg();
    scratch.with_files(&[("", "CON.txt", "content")]);

    let destination = scratch.path("out");
    std::fs::create_dir_all(&destination).unwrap();

    let refused = scratch.veil(&[
        "save-copy",
        &vault,
        "CON.txt",
        "--to",
        destination.to_str().unwrap(),
    ]);

    assert_eq!(refused.code, 14, "{}", refused.everything());
    assert!(
        refused.err.contains("CON.txt"),
        "the refusal did not name the file: {}",
        refused.err
    );
    assert!(
        std::fs::read_dir(&destination).unwrap().next().is_none(),
        "the destination folder was not empty after a refusal"
    );
}

/// T5.10 — An extraction to an exact, caller-chosen path is unaffected
/// (FR-31).
#[test]
fn t5_10_an_exact_destination_path_is_unaffected() {
    let scratch = Scratch::new("representable-exact-path");
    let vault = scratch.vault_arg();
    scratch.with_files(&[("", "CON.txt", "content")]);

    let destination = scratch.path("chosen-name.txt");
    let saved = scratch.veil(&[
        "save-copy",
        &vault,
        "CON.txt",
        "--to",
        destination.to_str().unwrap(),
    ]);

    assert_eq!(saved.code, 0, "{}", saved.everything());
    assert_eq!(scratch.read("chosen-name.txt"), "content");
}
