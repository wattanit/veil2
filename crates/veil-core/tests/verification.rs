//! Phase 2 test cases T2.37 through T2.40 — whole-vault verification
//! (FR-14, FR-33, S-4, Spec §4.8).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use harness::{Recorder, SMALL_CAP, add, create, open, pattern};
use veil_core::vault::Outcome;
use veil_core::{Cancel, Damaged, NoProgress};

/// T2.37 — verification passes on an intact vault and writes nothing
/// (FR-33, Spec §4.8).
///
/// Including the index slots: a verification that advanced a generation would
/// make the operation a write, and §4.8 requires it to run on a read-only
/// vault.
#[test]
fn t2_37_verification_passes_and_writes_nothing() {
    let scratch = harness::Scratch::new("verify-clean");
    let dir = scratch.vault_dir();

    let mut vault = create(&dir, SMALL_CAP);
    for i in 0..6 {
        add(
            &mut vault,
            &format!("f{i}.bin"),
            "d",
            &pattern(1500 + i * 400),
        );
    }
    assert!(veil_core::store::existing_pack_ids(&dir).unwrap().len() > 1);

    let before = harness::snapshot(&dir);
    let generation = vault.generation();

    let report = vault.verify(&mut NoProgress, &Cancel::new()).unwrap();
    assert!(report.complete);
    assert!(report.all_passed());
    assert_eq!(report.verdicts.len(), 6);
    assert!(report.failures().is_empty());

    assert_eq!(vault.generation(), generation);
    assert_eq!(
        harness::snapshot(&dir),
        before,
        "verification wrote to the vault"
    );
}

/// T2.38 — verification names every failure and stops at none (FR-33, S-4).
///
/// One damaged region must yield a complete list of what it cost — not a
/// superset, not the first casualty. A partial failure is presented as a list
/// of unreadable files rather than as a failure of the vault.
#[test]
fn t2_38_verification_names_every_failure_and_stops_at_none() {
    let scratch = harness::Scratch::new("verify-damage");
    let dir = scratch.vault_dir();

    let mut vault = create(&dir, SMALL_CAP);
    let mut ids = Vec::new();
    for i in 0..6 {
        ids.push(add(&mut vault, &format!("f{i}.bin"), "d", &pattern(3000)));
    }
    drop(vault);

    // Damage two entries in different packs, leaving the rest untouched.
    let vault = open(&dir).unwrap();
    let first = *ids.first().unwrap();
    let last = *ids.last().unwrap();
    let targets: Vec<_> = [first, last]
        .into_iter()
        .map(|id| {
            let entry = vault.entries().iter().find(|e| e.id == id).unwrap();
            (id, entry.extents[0])
        })
        .collect();
    assert_ne!(
        targets[0].1.pack_id, targets[1].1.pack_id,
        "the two targets must live in different packs"
    );
    drop(vault);

    for (_, extent) in &targets {
        harness::flip_byte_in_pack(&dir, extent.pack_id, extent.offset + 4);
    }

    let vault = open(&dir).unwrap();
    let report = vault.verify(&mut NoProgress, &Cancel::new()).unwrap();
    assert!(report.complete);
    assert_eq!(report.verdicts.len(), 6);

    let mut failed = report.failures();
    failed.sort_by_key(|id| id.get());
    let mut expected = vec![first, last];
    expected.sort_by_key(|id| id.get());
    assert_eq!(
        failed, expected,
        "verification named the wrong set of entries"
    );

    // Everything else passed, and still extracts. Damage costs only what it
    // touches.
    for id in &ids {
        if expected.contains(id) {
            continue;
        }
        assert_eq!(harness::read_back(&vault, *id).unwrap(), pattern(3000));
    }
    drop(vault);

    // A pack that is missing entirely is total damage to that pack, not a
    // broken vault: the vault opens, the entries with extents in it are
    // reported unreadable and named, and every other entry stays retrievable.
    let doomed_pack = targets[0].1.pack_id;
    let doomed_entries = {
        let vault = open(&dir).unwrap();
        veil_core::store::entries_in_pack(vault.entries(), doomed_pack)
    };
    std::fs::remove_file(veil_core::store::pack_path(&dir, doomed_pack)).unwrap();

    let vault = open(&dir).expect("a vault with a missing pack still opens");
    let report = vault.verify(&mut NoProgress, &Cancel::new()).unwrap();
    assert_eq!(report.verdicts.len(), 6);

    for verdict in &report.verdicts {
        if doomed_entries.contains(&verdict.id) {
            assert_eq!(
                verdict.outcome,
                Outcome::Failed(Damaged::Pack { id: doomed_pack }),
                "the entry was not attributed to the pack that failed"
            );
        }
    }
    let survivors: Vec<_> = report
        .verdicts
        .iter()
        .filter(|v| v.outcome == Outcome::Passed)
        .map(|v| v.id)
        .collect();
    assert!(
        !survivors.is_empty(),
        "one damaged pack must not cost the whole vault"
    );
}

/// T2.39 — a cancelled verification returns what it verified
/// (Spec §4.8, FR-14).
///
/// A partial verification is a partial answer, not a discarded one. Discarding
/// it makes cancellation cost the user everything they had already waited for.
#[test]
fn t2_39_a_cancelled_verification_returns_what_it_verified() {
    let scratch = harness::Scratch::new("verify-cancel");
    let dir = scratch.vault_dir();

    let mut vault = create(&dir, SMALL_CAP);
    for i in 0..8 {
        add(&mut vault, &format!("f{i}.bin"), "d", &pattern(2000));
    }

    let cancel = Cancel::new();
    let mut sink = harness::CancelAt::new(cancel.clone(), 3);
    let report = vault.verify(&mut sink, &cancel).unwrap();

    assert!(!report.complete, "a cancelled report claimed completeness");
    assert!(
        !report.verdicts.is_empty(),
        "the partial answer was discarded"
    );
    assert!(report.verdicts.len() < 8);
    assert!(report.all_passed());

    // `all_passed` on an incomplete report is not "the vault is sound", and the
    // report says which it is — an unexamined entry has no verdict, and
    // reporting one would be a guess.
    let examined: Vec<_> = report.verdicts.iter().map(|v| v.id).collect();
    assert!(vault.entries().len() > examined.len());
}

/// T2.40 — verification runs on a read-only vault (Spec §4.8, FR-33).
///
/// Requiring the ability to write would make the operation that diagnoses a
/// failing drive the one operation a failing drive cannot run.
#[test]
#[cfg(unix)]
fn t2_40_verification_runs_on_a_read_only_vault() {
    use std::os::unix::fs::PermissionsExt;

    let scratch = harness::Scratch::new("verify-readonly");
    let dir = scratch.vault_dir();

    let mut vault = create(&dir, SMALL_CAP);
    for i in 0..4 {
        add(&mut vault, &format!("f{i}.bin"), "d", &pattern(1000));
    }
    drop(vault);

    // Deny writing to the directory and to every file in it, which is what a
    // write-protected drive or a mounted image looks like.
    let mut denied = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_file() {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
            denied.push(path);
        }
    }
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    // Running as root defeats file permissions entirely. A case that silently
    // passes when it could not run is worse than one that does not run.
    if std::fs::File::create(dir.join("probe")).is_ok() {
        let _ = std::fs::remove_file(dir.join("probe"));
        restore(&dir, &denied);
        eprintln!("T2.40 skipped: this account can write regardless of permissions");
        return;
    }

    let vault = harness::open(&dir).expect("a read-only vault opens");
    assert_eq!(vault.access(), veil_core::vault::Access::ReadOnly);

    let report = vault
        .verify(&mut Recorder::default(), &Cancel::new())
        .unwrap();
    assert!(report.complete && report.all_passed());
    assert_eq!(report.verdicts.len(), 4);

    // Reads work; writes are refused rather than attempted and half-done.
    let id = vault.entries()[0].id;
    assert_eq!(harness::read_back(&vault, id).unwrap(), pattern(1000));
    drop(vault);

    let mut vault = harness::open(&dir).unwrap();
    assert!(
        vault
            .add(
                "new.bin",
                "d",
                &mut pattern(10).as_slice(),
                &mut NoProgress,
                &Cancel::new()
            )
            .is_err()
    );
    drop(vault);

    restore(&dir, &denied);
}

/// Puts the permissions back, so the scratch directory can be removed.
#[cfg(unix)]
fn restore(dir: &std::path::Path, files: &[std::path::PathBuf]) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
    for path in files {
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644));
    }
}
