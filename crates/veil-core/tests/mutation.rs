//! Phase 2 test cases T2.20 through T2.27 — replace, delete, and statistics
//! (FR-8, FR-13, FR-21, FR-22, FR-29, HC-4, S-2, Spec §3.2, §4.3, §4.5, §4.6).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use std::io::Read;

use harness::{SMALL_CAP, add, assert_statistics_match_recount, create, open, pattern};
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
    let mut vault = create(&dir, SMALL_CAP);

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
    let mut vault = create(&dir, SMALL_CAP);

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
    assert_statistics_match_recount(&vault, "after a failed replace");

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
    assert_statistics_match_recount(&vault, "after a cancelled replace");
}

/// T2.22 — replace makes the old content reclaimable in the same step
/// (FR-13, FR-21, FR-8).
///
/// Two generation steps would be the window HC-4 forbids.
#[test]
fn t2_22_replace_reclaims_the_old_content_in_one_step() {
    let scratch = harness::Scratch::new("replace-reclaim");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);

    let original = pattern(3000);
    let old_id = add(&mut vault, "doc.bin", "d", &original);
    let old_stored: u64 = vault
        .entries()
        .iter()
        .find(|e| e.id == old_id)
        .unwrap()
        .extents
        .iter()
        .map(|x| x.length)
        .sum();

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
    assert_eq!(
        after.reclaimable_bytes,
        before.reclaimable_bytes + old_stored
    );
    assert_statistics_match_recount(&vault, "after a replace");
}

/// T2.23 — a deleted entry is immediately unreachable (FR-21).
#[test]
fn t2_23_a_deleted_entry_is_immediately_unreachable() {
    let scratch = harness::Scratch::new("delete-unreachable");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);

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

/// T2.24 — delete accounts for what it did not erase (FR-21, FR-8, FR-29).
///
/// The honesty clause, asserted: the bytes are still there, and the figure the
/// user is shown says so. A user who deletes a file and then hands the vault to
/// someone else must not believe those bytes are gone.
#[test]
fn t2_24_delete_accounts_for_what_it_did_not_erase() {
    let scratch = harness::Scratch::new("delete-accounting");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);

    add(&mut vault, "kept.bin", "d", &pattern(1200));
    let gone = add(&mut vault, "gone.bin", "d", &pattern(3400));

    let stored: u64 = vault
        .entries()
        .iter()
        .find(|e| e.id == gone)
        .unwrap()
        .extents
        .iter()
        .map(|x| x.length)
        .sum();
    let before = vault.statistics();
    let packs_before: Vec<u64> = veil_core::store::existing_pack_ids(&dir)
        .unwrap()
        .into_iter()
        .map(|id| {
            std::fs::metadata(veil_core::store::pack_path(&dir, id))
                .unwrap()
                .len()
        })
        .collect();

    vault.delete(gone).unwrap();
    let after = vault.statistics();

    assert_eq!(after.entry_count, before.entry_count - 1);
    assert_eq!(after.logical_bytes, before.logical_bytes - 3400);
    assert_eq!(
        after.physical_bytes, before.physical_bytes,
        "delete must not pretend the bytes left"
    );
    assert_eq!(after.reclaimable_bytes, before.reclaimable_bytes + stored);

    let packs_after: Vec<u64> = veil_core::store::existing_pack_ids(&dir)
        .unwrap()
        .into_iter()
        .map(|id| {
            std::fs::metadata(veil_core::store::pack_path(&dir, id))
                .unwrap()
                .len()
        })
        .collect();
    assert_eq!(
        packs_before, packs_after,
        "delete rewrote a pack; that is compaction's job (FR-23)"
    );
    assert_statistics_match_recount(&vault, "after a delete");
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
    let mut vault = create(&dir, SMALL_CAP);

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

/// T2.26 — statistics match a full recount (FR-8, FR-22).
///
/// Checked after *each* operation. Checking only at the end lets two errors
/// cancel; checking after each names the operation that diverged.
#[test]
fn t2_26_statistics_match_a_full_recount() {
    let scratch = harness::Scratch::new("statistics");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);
    assert_statistics_match_recount(&vault, "empty");

    let mut ids = Vec::new();
    for i in 0..6 {
        ids.push(add(
            &mut vault,
            &format!("f{i}.bin"),
            "d",
            &pattern(500 + i * 700),
        ));
        assert_statistics_match_recount(&vault, &format!("after add {i}"));
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
    assert_statistics_match_recount(&vault, "after replace");

    vault.delete(ids[0]).unwrap();
    assert_statistics_match_recount(&vault, "after the first delete");
    vault.delete(ids[4]).unwrap();
    assert_statistics_match_recount(&vault, "after the second delete");

    vault
        .replace(
            "d",
            "f5.bin",
            &mut pattern(20).as_slice(),
            &mut NoProgress,
            &Cancel::new(),
        )
        .unwrap();
    assert_statistics_match_recount(&vault, "after a shrinking replace");

    // And across a reopen, since the figures live in the index rather than in
    // the process.
    let expected = vault.statistics();
    drop(vault);
    let vault = open(&dir).unwrap();
    assert_eq!(vault.statistics(), expected);
    assert_statistics_match_recount(&vault, "after a reopen");
}

/// T2.27 — statistics are available at open without reading content
/// (FR-22, S-2).
///
/// Deriving reclaimable space by scanning would cost more than the compaction
/// it advises. Asserted the strong way: the packs are removed, and the figures
/// are still correct — nothing that scanned them could survive it.
#[test]
fn t2_27_statistics_are_available_at_open_without_reading_content() {
    let scratch = harness::Scratch::new("statistics-at-open");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);

    for i in 0..5 {
        add(&mut vault, &format!("f{i}.bin"), "d", &pattern(2000));
    }
    let deleted = vault.entries()[1].id;
    vault.delete(deleted).unwrap();
    let expected = vault.statistics();
    assert!(expected.reclaimable_bytes > 0);
    drop(vault);

    for id in veil_core::store::existing_pack_ids(&dir).unwrap() {
        std::fs::remove_file(veil_core::store::pack_path(&dir, id)).unwrap();
    }

    let vault = open(&dir).unwrap();
    assert_eq!(vault.statistics(), expected);
}
