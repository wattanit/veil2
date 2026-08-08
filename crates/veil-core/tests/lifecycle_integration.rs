//! Phase 2 test case T2.34 — the full lifecycle with no terminal present
//! (A-1, A-4, Spec §9).
//!
//! **This is the phase's exit condition.** The original Veil had fourteen unit
//! tests, no integration tests, and logic that could not be exercised without a
//! pseudo-terminal. Everything below runs in one process, through the public
//! API, with nothing spawned and nothing prompted.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use harness::{Recorder, SMALL_CAP, assert_monotonic, create, pattern};
use veil_core::crypto::KdfParams;
use veil_core::vault::Unit;
use veil_core::vault::Vault;
use veil_core::{Cancel, NoProgress};

/// T2.34 — create, add, browse, extract, replace, delete, report, verify,
/// change the password, and lock (A-1, A-4).
#[test]
fn t2_34_the_full_lifecycle_runs_with_no_terminal_present() {
    let scratch = harness::Scratch::new("lifecycle");
    let dir = scratch.vault_dir();
    let cancel = Cancel::new();

    // A source tree on disk, so the folder path is exercised rather than only
    // the byte-slice one a test finds convenient.
    let tree = scratch.path("sources");
    std::fs::create_dir_all(tree.join("photos/2024")).unwrap();
    std::fs::write(tree.join("notes.txt"), pattern(400)).unwrap();
    std::fs::write(tree.join("photos/one.jpg"), pattern(5000)).unwrap();
    std::fs::write(tree.join("photos/2024/two.jpg"), pattern(9000)).unwrap();

    // Create.
    let mut vault = create(&dir, SMALL_CAP);
    assert_eq!(vault.statistics().entry_count, 0);

    // Add a single file and a folder, watching progress. One sink per
    // operation: monotonicity is a property of an operation, and a sink shared
    // across two would only be asserting that the second started after the
    // first finished.
    let mut single_progress = Recorder::default();
    let single = vault
        .add(
            "readme.md",
            "",
            &mut pattern(1200).as_slice(),
            &mut single_progress,
            &cancel,
        )
        .unwrap();
    assert_monotonic(&single_progress.0, "single-file ingest");
    assert!(single_progress.0.iter().all(|r| r.unit == Unit::Bytes));

    let mut folder_progress = Recorder::default();
    let folder = vault
        .add_folder(&tree, &mut folder_progress, &cancel)
        .unwrap();
    assert_eq!(folder.added.len(), 3);
    assert!(folder.skipped.is_empty());
    assert_monotonic(&folder_progress.0, "folder ingest");
    // A folder ingest counts entries: a bar that restarts at every file is
    // worse than no bar.
    assert!(folder_progress.0.iter().all(|r| r.unit == Unit::Entries));
    assert_eq!(folder_progress.0.last().unwrap().done, 3);

    // Browse: from memory, no file read.
    assert_eq!(vault.entries().len(), 4);
    assert!(vault.find("photos/2024", "two.jpg").is_some());
    harness::assert_statistics_match_recount(&vault, "after ingest");

    // Extract, to a caller-supplied destination.
    let destination = scratch.path("recovered.jpg");
    let two = vault.find("photos/2024", "two.jpg").unwrap().id;
    vault
        .extract_to_path(two, &destination, &mut NoProgress, &cancel)
        .unwrap();
    assert_eq!(std::fs::read(&destination).unwrap(), pattern(9000));

    // Replace, matched on the full path.
    let replaced = vault
        .replace(
            "photos",
            "one.jpg",
            &mut pattern(7000).as_slice(),
            &mut NoProgress,
            &cancel,
        )
        .unwrap();
    assert_eq!(harness::read_back(&vault, replaced).unwrap(), pattern(7000));

    // Delete, with the accounting that says the bytes are still there.
    let before = vault.statistics();
    vault.delete(single).unwrap();
    let after = vault.statistics();
    assert_eq!(after.entry_count, before.entry_count - 1);
    assert!(after.reclaimable_bytes > before.reclaimable_bytes);
    harness::assert_statistics_match_recount(&vault, "after the lifecycle mutations");

    // Verify.
    let mut verify_progress = Recorder::default();
    let report = vault.verify(&mut verify_progress, &cancel).unwrap();
    assert!(report.complete && report.all_passed());
    assert_eq!(report.verdicts.len(), 3);

    // Change the password.
    let new = harness::other_password("lifecycle");
    vault
        .change_password(&harness::password(), &new, KdfParams::for_tests())
        .unwrap();

    // Lock, then reopen under the new password and confirm everything survived.
    vault.lock();

    let reopened = Vault::open(&dir, &new).expect("the vault reopens under the new password");
    assert_eq!(reopened.entries().len(), 3);
    assert_eq!(harness::read_back(&reopened, two).unwrap(), pattern(9000));
    assert!(
        reopened
            .verify(&mut NoProgress, &cancel)
            .unwrap()
            .all_passed()
    );
    assert_eq!(reopened.statistics(), after);
}
