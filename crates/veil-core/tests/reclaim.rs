//! Phase 4 test cases T4.9 to T4.14 and T4.25 — reclaiming space
//! (Spec §4.5; FR-23, FR-24, FR-25, S-4).
//!
//! Every case here uses a small pack cap. That the cap is a value the API
//! accepts rather than a constant is what makes multi-pack behaviour testable
//! in kilobytes, and multi-pack behaviour is the whole of this subject.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use harness::{
    CancelAt, SMALL_CAP, add, assert_statistics_match_recount, create, open, pattern, read_back,
};
use veil_core::index::EntryId;
use veil_core::store::{existing_pack_ids, pack_path, total_pack_bytes};
use veil_core::vault::{Cancel, NoProgress, Vault};
use veil_core::{Damaged, Error};

/// Builds a vault of several files, each big enough to want a pack of its own
/// at [`SMALL_CAP`], and returns what was stored so it can be compared after.
fn stocked(dir: &std::path::Path, count: usize) -> (Vault, BTreeMap<String, (EntryId, Vec<u8>)>) {
    let mut vault = create(dir, SMALL_CAP);
    let mut stored = BTreeMap::new();
    for n in 0..count {
        let name = format!("f{n}.bin");
        let content = pattern(3000 + n * 97);
        let id = add(&mut vault, &name, "d", &content);
        stored.insert(name, (id, content));
    }
    (vault, stored)
}

fn delete_by_name(vault: &mut Vault, stored: &BTreeMap<String, (EntryId, Vec<u8>)>, name: &str) {
    vault.delete(stored[name].0).unwrap();
}

/// Every live file still reads back exactly as it was stored.
fn assert_all_intact(vault: &Vault, stored: &BTreeMap<String, (EntryId, Vec<u8>)>, label: &str) {
    for entry in vault.entries() {
        let (id, expected) = &stored[&entry.name];
        assert_eq!(entry.id, *id, "{label}: {} was reissued", entry.name);
        assert_eq!(
            &read_back(vault, *id).unwrap(),
            expected,
            "{label}: {} came back different",
            entry.name
        );
    }
}

/// T4.9 — what was promised is what is recovered (FR-8, FR-25, Design §8.4).
///
/// The reclaimable figure is what Design §8.4 puts in the control the user
/// presses. An operation that recovers less than it said has made that number
/// untrue, which is why this asserts equality rather than "some space".
#[test]
fn t4_9_what_was_promised_is_what_is_recovered() {
    let scratch = harness::Scratch::new("reclaim-promised");
    let dir = scratch.vault_dir();
    let (mut vault, stored) = stocked(&dir, 5);

    delete_by_name(&mut vault, &stored, "f1.bin");
    delete_by_name(&mut vault, &stored, "f3.bin");

    let before = vault.statistics();
    assert!(before.reclaimable_bytes > 0, "nothing to reclaim");

    let reclaimed = vault.compact(&mut NoProgress, &Cancel::new()).unwrap();
    let after = vault.statistics();

    assert!(reclaimed.complete);
    assert_eq!(
        reclaimed.bytes_recovered, before.reclaimable_bytes,
        "recovered a different amount than the figures promised"
    );
    assert_eq!(after.reclaimable_bytes, 0);
    assert_eq!(
        after.physical_bytes,
        before.physical_bytes - reclaimed.bytes_recovered
    );
    assert_eq!(after.entry_count, before.entry_count);
    assert_eq!(after.logical_bytes, before.logical_bytes);
    assert_statistics_match_recount(&vault, "after reclaiming");
    assert_all_intact(&vault, &stored, "after reclaiming");
}

/// T4.10 — working space stays bounded by about one pack (FR-25).
///
/// The requirement that makes reclaiming possible at the sizes in §1. An
/// implementation that copies everything and swaps at the end passes every
/// other case in this file and fails this one.
#[test]
fn t4_10_working_space_stays_bounded_by_about_one_pack() {
    let scratch = harness::Scratch::new("reclaim-bounded");
    let dir = scratch.vault_dir();
    let (mut vault, stored) = stocked(&dir, 8);

    // Garbage in every pack, so a whole-vault rewrite is the tempting shortcut.
    for name in ["f0.bin", "f2.bin", "f4.bin", "f6.bin"] {
        delete_by_name(&mut vault, &stored, name);
    }

    let start = total_pack_bytes(&dir).unwrap();

    // Sampled from another thread rather than from the progress sink. The sink
    // is called between packs, which is *after* the old pack has gone — the
    // moment worth measuring is while the new pack and the old one both exist,
    // and only polling the filesystem catches it. The sampler touches no vault
    // state, so it observes without participating.
    let stop = Arc::new(AtomicBool::new(false));
    let peak = Arc::new(AtomicU64::new(start));
    let sampler = {
        let (dir, stop, peak) = (dir.clone(), Arc::clone(&stop), Arc::clone(&peak));
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                if let Ok(now) = total_pack_bytes(&dir) {
                    peak.fetch_max(now, Ordering::Relaxed);
                }
            }
        })
    };

    vault.compact(&mut NoProgress, &Cancel::new()).unwrap();
    stop.store(true, Ordering::Relaxed);
    sampler.join().unwrap();

    let peak = peak.load(Ordering::Relaxed);
    assert!(
        peak <= start + SMALL_CAP,
        "peak on disk was {peak} against a start of {start}, more than one pack of headroom"
    );
    assert!(
        peak > total_pack_bytes(&dir).unwrap(),
        "the sampler never saw the operation in flight, so it measured nothing"
    );
    assert_all_intact(&vault, &stored, "after a bounded reclaim");
}

/// T4.11 — live content survives byte-identically, and so do its identifiers
/// (Spec §3.3, §4.5, FR-25).
///
/// The entry identifier is bound into the key wrapping and the content's
/// associated data. Reissuing one during what is meant to be housekeeping would
/// be a cryptographic fault, and this is where it would show.
#[test]
fn t4_11_live_content_and_identifiers_survive() {
    let scratch = harness::Scratch::new("reclaim-identity");
    let dir = scratch.vault_dir();
    let (mut vault, stored) = stocked(&dir, 6);

    let hashes: BTreeMap<String, [u8; 32]> = vault
        .entries()
        .iter()
        .map(|e| (e.name.clone(), e.content_hash))
        .collect();

    delete_by_name(&mut vault, &stored, "f2.bin");
    vault.compact(&mut NoProgress, &Cancel::new()).unwrap();

    assert_all_intact(&vault, &stored, "after reclaiming");
    for entry in vault.entries() {
        assert_eq!(
            entry.content_hash, hashes[&entry.name],
            "{}'s content hash changed",
            entry.name
        );
    }

    // And across a reopen, since the extents that moved live in the index.
    drop(vault);
    let vault = open(&dir).unwrap();
    assert_all_intact(&vault, &stored, "after a reopen");
    assert_statistics_match_recount(&vault, "after a reopen");
}

/// T4.12 — a pack with nothing to recover is not rewritten (FR-25).
///
/// Copying a pack to recover nothing is pure cost, and at the sizes in §1 it is
/// minutes of it.
#[test]
fn t4_12_a_pack_with_no_garbage_is_not_rewritten() {
    let scratch = harness::Scratch::new("reclaim-untouched");
    let dir = scratch.vault_dir();
    let (mut vault, stored) = stocked(&dir, 4);

    // The pack holding the last file keeps everything it has.
    let untouched = vault
        .entries()
        .iter()
        .find(|e| e.name == "f3.bin")
        .unwrap()
        .extents[0]
        .pack_id;
    let before = std::fs::read(pack_path(&dir, untouched)).unwrap();

    delete_by_name(&mut vault, &stored, "f0.bin");
    let reclaimed = vault.compact(&mut NoProgress, &Cancel::new()).unwrap();

    assert_eq!(
        reclaimed.packs_rewritten, 1,
        "more packs were rewritten than had garbage in them"
    );
    assert_eq!(
        std::fs::read(pack_path(&dir, untouched)).unwrap(),
        before,
        "a pack with no garbage in it was rewritten anyway"
    );
}

/// T4.13 — cancelling keeps what was already reclaimed (FR-14, FR-24).
///
/// Each pack is its own transaction, so stopping costs at most the pack in
/// flight. That is FR-24's "at most the current unit of work" made observable.
#[test]
fn t4_13_cancelling_keeps_what_was_already_reclaimed() {
    let scratch = harness::Scratch::new("reclaim-cancel");
    let dir = scratch.vault_dir();
    let (mut vault, stored) = stocked(&dir, 8);

    for name in ["f0.bin", "f2.bin", "f4.bin", "f6.bin"] {
        delete_by_name(&mut vault, &stored, name);
    }
    let before = vault.statistics();

    let cancel = Cancel::new();
    let mut sink = CancelAt::new(cancel.clone(), 1);
    let reclaimed = vault.compact(&mut sink, &cancel).unwrap();

    assert!(!reclaimed.complete, "the cancellation was not noticed");
    assert!(
        vault.statistics().reclaimable_bytes <= before.reclaimable_bytes,
        "reclaimable space grew during a cancelled reclaim"
    );
    assert_all_intact(&vault, &stored, "after a cancelled reclaim");

    // What it had already reclaimed stays reclaimed, and the leftover of the
    // pack in flight is residue the next open clears (FR-32).
    drop(vault);
    let vault = open(&dir).unwrap();
    assert_all_intact(&vault, &stored, "after reopening a cancelled reclaim");
    assert_statistics_match_recount(&vault, "after reopening a cancelled reclaim");

    // And running it again finishes the job.
    let mut vault = vault;
    let rest = vault.compact(&mut NoProgress, &Cancel::new()).unwrap();
    assert!(rest.complete);
    assert_eq!(vault.statistics().reclaimable_bytes, 0);
    assert_all_intact(&vault, &stored, "after finishing the reclaim");
}

/// T4.14 — a pack that is not all there is refused, not compacted away
/// (S-4, HC-3).
///
/// Copying a short extent forward would produce an entry whose recorded length
/// no longer matches its stored bytes, and would delete the original that
/// proved what happened.
#[test]
fn t4_14_a_short_pack_is_refused() {
    let scratch = harness::Scratch::new("reclaim-short");
    let dir = scratch.vault_dir();
    let (mut vault, stored) = stocked(&dir, 4);
    delete_by_name(&mut vault, &stored, "f0.bin");

    // A pack that still has something live in it, so truncating it leaves an
    // extent claiming more than the file holds.
    let victim = vault
        .entries()
        .iter()
        .find(|e| e.name == "f3.bin")
        .unwrap()
        .extents[0]
        .pack_id;
    drop(vault);

    let path = pack_path(&dir, victim);
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
    let packs_before = existing_pack_ids(&dir).unwrap();

    let mut vault = open(&dir).unwrap();
    let outcome = vault.compact(&mut NoProgress, &Cancel::new());

    match outcome {
        Err(Error::Corrupt { what, affected }) => {
            assert_eq!(what, Damaged::Pack { id: victim });
            assert!(
                !affected.is_empty(),
                "the refusal named no files, so it says nothing about what it costs (S-4)"
            );
        }
        other => panic!("a short pack was not refused: {other:?}"),
    }

    // Nothing was written at all: the refusal comes before any byte moves, so
    // a damaged vault is not half-reorganised on the way to being refused.
    assert_eq!(
        existing_pack_ids(&dir).unwrap(),
        packs_before,
        "the refusal still rearranged the vault"
    );
    assert!(
        pack_path(&dir, victim).exists(),
        "the damaged pack was removed by the operation that refused it"
    );
}

/// T4.25 — bounded working space at a size where it is unambiguous (FR-25).
///
/// T4.10 asserts the same property where a bug could hide in the noise. This is
/// the Plan's exit condition, and it is ignored by default because it costs
/// minutes and disk.
#[test]
#[ignore = "costs minutes and disk; the Plan's exit condition, run on request"]
fn t4_25_bounded_working_space_at_scale() {
    const CAP: u64 = 8 * 1024 * 1024;
    const FILES: usize = 40;

    let scratch = harness::Scratch::new("reclaim-scale");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, CAP);

    let content = pattern(6 * 1024 * 1024);
    let mut ids = Vec::new();
    for n in 0..FILES {
        ids.push(add(&mut vault, &format!("big{n}.bin"), "d", &content));
    }

    // Most of it garbage, so a whole-vault rewrite would be the shortcut.
    for id in ids.iter().take(FILES * 3 / 4) {
        vault.delete(*id).unwrap();
    }

    let start = total_pack_bytes(&dir).unwrap();
    let reclaimed = vault.compact(&mut NoProgress, &Cancel::new()).unwrap();
    let end = total_pack_bytes(&dir).unwrap();

    assert!(reclaimed.complete);
    assert!(
        end + reclaimed.bytes_recovered == start,
        "recovered {} but the vault fell from {start} to {end}",
        reclaimed.bytes_recovered
    );
    assert!(
        start > 20 * CAP,
        "the fixture is too small for one pack of headroom to be a meaningful bound"
    );
    assert_statistics_match_recount(&vault, "after reclaiming at scale");
}
