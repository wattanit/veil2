//! Phase 2 test cases T2.20 through T2.27 — replace, delete, and statistics
//! (FR-7, FR-13, FR-22, HC-4, S-2, Spec §3.2, §4.3, §4.5, §4.6).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use std::io::Read;

use harness::{add, assert_statistics_correct, create, open, pattern};
use veil_core::{Cancel, Error, NoProgress};

/// A source that fails partway, standing in for a disk that goes away.
struct FailsAfter(usize);

impl Read for FailsAfter {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.0 == 0 {
            return Err(std::io::Error::other("the source went away"));
        }
        let n = buf.len().min(self.0);
        for (i, slot) in buf[..n].iter_mut().enumerate() {
            *slot = (i % 251) as u8;
        }
        self.0 -= n;
        Ok(n)
    }
}

/// T2.20 — replace matches on the full path, never on the name alone
/// (FR-13, Spec §4.6).
///
/// Matching on name alone would let an ingest into one folder silently
/// overwrite a file in another.
#[test]
fn t2_20_replace_matches_on_the_full_path() {
    let scratch = harness::Scratch::new("replace-path");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir);

    let work = pattern(1000);
    let personal = pattern(1500);
    let work_id = add(&mut vault, "report.pdf", "work/2024", &work);
    let personal_id = add(&mut vault, "report.pdf", "personal", &personal);

    let replacement = pattern(2000);
    let new_id = vault
        .replace(
            "work/2024",
            "report.pdf",
            &mut replacement.as_slice(),
            &mut NoProgress,
            &Cancel::new(),
        )
        .unwrap();

    assert_ne!(new_id, work_id, "a replace issues a new identifier");
    assert_eq!(vault.entries().len(), 2);

    let replaced = vault.find("work/2024", "report.pdf").unwrap();
    assert_eq!(replaced.id, new_id);
    assert_eq!(harness::read_back(&vault, new_id).unwrap(), replacement);

    // The same-named file in another folder is byte-identical to before.
    let untouched = vault.find("personal", "report.pdf").unwrap();
    assert_eq!(untouched.id, personal_id);
    assert_eq!(harness::read_back(&vault, personal_id).unwrap(), personal);

    // The old entry is gone, so nothing reaches the previous content.
    assert!(vault.entries().iter().all(|e| e.id != work_id));

    // A path that does not exist is a refusal, not a silent insert.
    assert!(
        vault
            .replace(
                "nowhere",
                "report.pdf",
                &mut pattern(10).as_slice(),
                &mut NoProgress,
                &Cancel::new(),
            )
            .is_err()
    );
    assert_eq!(vault.entries().len(), 2);
}

/// T2.21 — there is never a moment with zero intact versions (FR-13, HC-4).
///
/// A remove-then-add implementation passes every test that does not fail in the
/// middle. These are the ones that fail in the middle.
#[test]
fn t2_21_a_failed_replace_leaves_the_original_intact() {
    let scratch = harness::Scratch::new("replace-failure");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir);

    let original = pattern(4000);
    let id = add(&mut vault, "doc.bin", "d", &original);
    let generation = vault.generation();
    let stats = vault.statistics();

    // A source that dies partway.
    let failed = vault.replace(
        "d",
        "doc.bin",
        &mut FailsAfter(2000),
        &mut NoProgress,
        &Cancel::new(),
    );
    assert!(failed.is_err(), "the replace should not have succeeded");

    assert_eq!(vault.generation(), generation, "a generation was consumed");
    assert_eq!(vault.statistics(), stats);
    assert_eq!(vault.find("d", "doc.bin").unwrap().id, id);
    assert_eq!(harness::read_back(&vault, id).unwrap(), original);
    assert_statistics_correct(&vault, "after a failed replace");

    // And a cancelled one.
    let cancel = Cancel::new();
    let mut sink = harness::CancelAt::new(cancel.clone(), 1);
    let content = pattern(veil_core::crypto::CHUNK_LEN * 3);
    let cancelled = vault.replace("d", "doc.bin", &mut content.as_slice(), &mut sink, &cancel);
    assert!(matches!(
        cancelled,
        Err(Error::Cancelled { rolled_back: true })
    ));

    assert_eq!(vault.generation(), generation);
    assert_eq!(vault.find("d", "doc.bin").unwrap().id, id);
    assert_eq!(harness::read_back(&vault, id).unwrap(), original);
    assert_statistics_correct(&vault, "after a cancelled replace");
}

/// T2.22 — replace advances the generation exactly once (FR-13).
///
/// Two generation steps would be the window HC-4 forbids. The old entry's
/// file is removed only after that one commit — never before, and never left
/// behind afterward.
#[test]
fn t2_22_replace_advances_the_generation_exactly_once() {
    let scratch = harness::Scratch::new("replace-generation");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir);

    let original = pattern(3000);
    let old_id = add(&mut vault, "doc.bin", "d", &original);

    let before = vault.statistics();
    let generation = vault.generation();

    let replacement = pattern(5000);
    vault
        .replace(
            "d",
            "doc.bin",
            &mut replacement.as_slice(),
            &mut NoProgress,
            &Cancel::new(),
        )
        .unwrap();

    assert_eq!(vault.generation(), generation + 1, "exactly one step");
    let after = vault.statistics();
    assert_eq!(after.entry_count, before.entry_count);
    assert_eq!(
        after.logical_bytes,
        before.logical_bytes - original.len() as u64 + replacement.len() as u64
    );
    assert!(
        !veil_core::store::exists(&dir, old_id),
        "the old entry's file was left behind after replace"
    );
    assert_statistics_correct(&vault, "after a replace");
}

/// T2.23 — a deleted entry is immediately unreachable (FR-22).
#[test]
fn t2_23_a_deleted_entry_is_immediately_unreachable() {
    let scratch = harness::Scratch::new("delete-unreachable");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir);

    let gone = add(&mut vault, "gone.bin", "d", &pattern(800));
    let kept = add(&mut vault, "kept.bin", "d", &pattern(900));

    vault.delete(gone).unwrap();

    assert!(vault.entries().iter().all(|e| e.id != gone));
    assert!(vault.find("d", "gone.bin").is_none());
    assert!(
        harness::read_back(&vault, gone).is_err(),
        "a deleted entry returned content"
    );
    assert_eq!(harness::read_back(&vault, kept).unwrap(), pattern(900));

    // Deleting it again is a refusal, not a second decrement.
    assert!(vault.delete(gone).is_err());
    assert_eq!(vault.statistics().entry_count, 1);

    // And it stays gone across a reopen: the index is what decides.
    drop(vault);
    let vault = open(&dir).unwrap();
    assert!(vault.entries().iter().all(|e| e.id != gone));
}

/// T2.24 — delete removes the entry's file immediately (FR-22, Spec §4.5).
///
/// There is no reclaimable figure to check: deleting already frees the space,
/// and the file is gone from `entries/` as soon as `delete` returns.
#[test]
fn t2_24_delete_removes_the_entrys_file_immediately() {
    let scratch = harness::Scratch::new("delete-frees-file");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir);

    add(&mut vault, "kept.bin", "d", &pattern(1200));
    let gone = add(&mut vault, "gone.bin", "d", &pattern(3400));
    assert!(veil_core::store::exists(&dir, gone));

    let before = vault.statistics();

    vault.delete(gone).unwrap();
    let after = vault.statistics();

    assert_eq!(after.entry_count, before.entry_count - 1);
    assert_eq!(after.logical_bytes, before.logical_bytes - 3400);
    assert!(
        !veil_core::store::exists(&dir, gone),
        "delete must free the entry's file immediately"
    );
    assert_statistics_correct(&vault, "after a delete");
}

/// T2.25 — entry identifiers are never reused (Spec §3.2, HC-3).
///
/// A reused identifier would let a wrapped key from a deleted entry decrypt
/// under a live one's nonce — a defect no functional test would ever surface,
/// which is exactly why it gets a test of its own.
#[test]
fn t2_25_entry_identifiers_are_never_reused() {
    let scratch = harness::Scratch::new("identifiers");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir);

    let mut ever_issued = Vec::new();
    for i in 0..3 {
        ever_issued.push(add(&mut vault, &format!("f{i}.bin"), "d", &pattern(100)));
    }

    // Delete the highest, then add. Deriving the next identifier from the
    // highest *live* one reissues it here.
    let highest = *ever_issued.iter().max_by_key(|id| id.get()).unwrap();
    vault.delete(highest).unwrap();
    let next = add(&mut vault, "after.bin", "d", &pattern(100));
    assert!(
        next.get() > highest.get(),
        "identifier {} was reissued after deleting {}",
        next.get(),
        highest.get()
    );
    ever_issued.push(next);

    // Now empty the vault entirely and add again. A counter derived from the
    // entries is at its most wrong when there are none.
    let all: Vec<_> = vault.entries().iter().map(|e| e.id).collect();
    for id in all {
        vault.delete(id).unwrap();
    }
    assert_eq!(vault.entries().len(), 0);

    let after_empty = add(&mut vault, "fresh.bin", "d", &pattern(100));
    let ceiling = ever_issued.iter().map(|id| id.get()).max().unwrap();
    assert!(
        after_empty.get() > ceiling,
        "identifier {} was reissued after emptying the vault",
        after_empty.get()
    );

    // The counter survives a reopen, or the guarantee lasts only as long as the
    // process does.
    ever_issued.push(after_empty);
    drop(vault);
    let mut vault = open(&dir).unwrap();
    let after_reopen = add(&mut vault, "reopened.bin", "d", &pattern(100));
    let ceiling = ever_issued.iter().map(|id| id.get()).max().unwrap();
    assert!(
        after_reopen.get() > ceiling,
        "identifier {} was reissued after a reopen",
        after_reopen.get()
    );
}

/// T2.26 — statistics match a direct sum after any sequence of operations
/// (FR-7, FR-22).
///
/// Checked after *each* operation. Checking only at the end lets two errors
/// cancel; checking after each names the operation that diverged.
#[test]
fn t2_26_statistics_match_a_direct_sum() {
    let scratch = harness::Scratch::new("statistics");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir);
    assert_statistics_correct(&vault, "empty");

    let mut ids = Vec::new();
    for i in 0..6 {
        ids.push(add(
            &mut vault,
            &format!("f{i}.bin"),
            "d",
            &pattern(500 + i * 700),
        ));
        assert_statistics_correct(&vault, &format!("after add {i}"));
    }

    vault
        .replace(
            "d",
            "f2.bin",
            &mut pattern(9000).as_slice(),
            &mut NoProgress,
            &Cancel::new(),
        )
        .unwrap();
    assert_statistics_correct(&vault, "after replace");

    vault.delete(ids[0]).unwrap();
    assert_statistics_correct(&vault, "after the first delete");
    vault.delete(ids[4]).unwrap();
    assert_statistics_correct(&vault, "after the second delete");

    vault
        .replace(
            "d",
            "f5.bin",
            &mut pattern(20).as_slice(),
            &mut NoProgress,
            &Cancel::new(),
        )
        .unwrap();
    assert_statistics_correct(&vault, "after a shrinking replace");

    // And across a reopen, since the figures are derived from the entries the
    // index holds rather than cached in the process.
    let expected = vault.statistics();
    drop(vault);
    let vault = open(&dir).unwrap();
    assert_eq!(vault.statistics(), expected);
    assert_statistics_correct(&vault, "after a reopen");
}

/// T2.27 — statistics are available at open without reading any entry file
/// (FR-7, S-2).
///
/// Asserted the strong way: every entry file is removed, and the figures are
/// still correct — nothing that read one could survive it.
#[test]
fn t2_27_statistics_are_available_at_open_without_reading_any_entry_file() {
    let scratch = harness::Scratch::new("statistics-at-open");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir);

    for i in 0..5 {
        add(&mut vault, &format!("f{i}.bin"), "d", &pattern(2000));
    }
    let deleted = vault.entries()[1].id;
    vault.delete(deleted).unwrap();
    let expected = vault.statistics();
    let ids: Vec<_> = vault.entries().iter().map(|e| e.id).collect();
    drop(vault);

    for id in ids {
        std::fs::remove_file(veil_core::store::entry_path(&dir, id)).unwrap();
    }

    let vault = open(&dir).unwrap();
    assert_eq!(vault.statistics(), expected);
}
