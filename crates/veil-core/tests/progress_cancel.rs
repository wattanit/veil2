//! Phase 2 test cases T2.6 through T2.9 — progress and cancellation
//! (A-3, FR-15, FR-18, FR-20, Spec §2, §4.8).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use std::sync::atomic::Ordering;

use harness::{CancelAt, CountingSource, Recorder, add, assert_monotonic, create, pattern};
use veil_core::crypto::CHUNK_LEN;
use veil_core::vault::Unit;
use veil_core::{Cancel, Error, NoProgress};

/// Several chunks, so a boundary is crossed more than once.
fn multi_chunk() -> Vec<u8> {
    pattern(CHUNK_LEN * 3 + 17)
}

/// T2.6 — progress is reported, monotonic, and complete
/// (A-3, FR-15, FR-20, Spec §4.8).
///
/// A sink called once at the end satisfies "reports progress" and is useless to
/// a progress bar, which is why monotonic growth is asserted rather than mere
/// presence.
#[test]
fn t2_6_progress_is_reported_monotonic_and_complete() {
    let scratch = harness::Scratch::new("progress");
    let dir = scratch.vault_dir();
    let content = multi_chunk();

    let mut vault = create(&dir);

    let mut ingest = Recorder::default();
    let id = vault
        .add(
            "big.bin",
            "d",
            &mut content.as_slice(),
            &mut ingest,
            &Cancel::new(),
        )
        .unwrap();
    assert_monotonic(&ingest.0, "ingest");
    assert!(ingest.0.len() > 1, "ingest reported only once");
    assert_eq!(ingest.0.last().unwrap().done, content.len() as u64);
    assert!(ingest.0.iter().all(|r| r.unit == Unit::Bytes));

    let mut extract = Recorder::default();
    let mut out = Vec::new();
    vault
        .extract(id, &mut out, &mut extract, &Cancel::new())
        .unwrap();
    assert_eq!(out, content);
    assert_monotonic(&extract.0, "extract");
    assert!(extract.0.len() > 1, "extraction reported only once");
    assert_eq!(extract.0.last().unwrap().done, content.len() as u64);
    // Extraction knows the size in advance, from the index, so the total is
    // available and a caller can show a bar rather than a spinner.
    assert_eq!(extract.0.last().unwrap().total, Some(content.len() as u64));

    // Verification counts entries, not bytes: the Design Guideline's estimate
    // is in time, and entry counts are what a user can hold in their head.
    add(&mut vault, "second.bin", "d", &pattern(500));
    let mut verify = Recorder::default();
    let report = vault.verify(&mut verify, &Cancel::new()).unwrap();
    assert!(report.all_passed() && report.complete);
    assert_monotonic(&verify.0, "verify");
    assert!(verify.0.iter().all(|r| r.unit == Unit::Entries));
    assert_eq!(verify.0.last().unwrap().done, 2);
    assert_eq!(verify.0.last().unwrap().total, Some(2));
}

/// T2.7 — a cancelled ingest leaves no trace in the vault
/// (FR-15, Spec §4.7).
#[test]
fn t2_7_a_cancelled_ingest_leaves_no_trace() {
    let scratch = harness::Scratch::new("cancel-ingest");
    let dir = scratch.vault_dir();

    let mut vault = create(&dir);
    add(&mut vault, "kept.bin", "d", &pattern(3000));

    let before_entries = vault.entries().len();
    let before_stats = vault.statistics();
    let before_generation = vault.generation();
    let before_files = harness::snapshot(&dir);

    let cancel = Cancel::new();
    let mut sink = CancelAt::new(cancel.clone(), CHUNK_LEN as u64);
    let content = multi_chunk();
    let outcome = vault.add(
        "doomed.bin",
        "d",
        &mut content.as_slice(),
        &mut sink,
        &cancel,
    );

    // Cancellation is its own outcome, carrying what it left behind. A caller
    // that cannot tell it from a failure cannot keep the Design Guideline's
    // promise that the vault is as it was.
    assert!(
        matches!(outcome, Err(Error::Cancelled { rolled_back: true })),
        "expected a rolled-back cancellation, got {outcome:?}"
    );

    assert_eq!(vault.entries().len(), before_entries);
    assert_eq!(vault.statistics(), before_stats);
    assert_eq!(vault.generation(), before_generation);
    harness::assert_statistics_correct(&vault, "after a cancelled ingest");

    // Indistinguishable from a vault where the operation never began, at the
    // level the API exposes: no entry, no statistics change, no generation
    // step. The cancelled write's own entry file may still be on disk as
    // unreferenced residue (Spec §4.5) — there is no rollback to run, because
    // nothing yet pointed at it, and nothing here sweeps it automatically.
    let after_files: Vec<_> = harness::snapshot(&dir)
        .into_iter()
        .filter(|(name, _)| !name.ends_with(".entry"))
        .collect();
    let before_non_entry: Vec<_> = before_files
        .into_iter()
        .filter(|(name, _)| !name.ends_with(".entry"))
        .collect();
    assert_eq!(
        after_files, before_non_entry,
        "a cancelled ingest changed something other than its own residue"
    );

    // And the vault is sound afterwards, which is what "as it was" has to mean.
    drop(vault);
    let vault = harness::open(&dir).unwrap();
    assert!(
        vault
            .verify(&mut NoProgress, &Cancel::new())
            .unwrap()
            .all_passed()
    );
}

/// T2.8 — cancellation takes effect within a bounded number of chunks
/// (Spec §2, FR-15).
///
/// **The bound is two chunks, not one, and that is a property of the
/// construction rather than a slack allowance.** Knowing which chunk is last
/// requires reading the next one first (STREAM tags the final chunk
/// differently), so a hook that stops at the boundary after chunk *n* has
/// already caused chunk *n+1* to be read. What FR-15 needs is that the bound is
/// a constant and not the file — that is what this asserts.
#[test]
fn t2_8_cancellation_takes_effect_within_a_bounded_number_of_chunks() {
    let scratch = harness::Scratch::new("cancel-latency");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir);

    let total = CHUNK_LEN * 8;
    let cancel = Cancel::new();
    let mut source = CountingSource::new(total);
    let taken = source.taken.clone();
    {
        let cancel = cancel.clone();
        let taken = taken.clone();
        source.on_read = Some(Box::new(move |_| {
            if taken.load(Ordering::SeqCst) >= CHUNK_LEN {
                cancel.cancel();
            }
        }));
    }

    let outcome = vault.add("huge.bin", "d", &mut source, &mut NoProgress, &cancel);
    assert!(matches!(
        outcome,
        Err(Error::Cancelled { rolled_back: true })
    ));

    let read = taken.load(Ordering::SeqCst);
    assert!(
        read <= CHUNK_LEN * 3,
        "cancellation read {read} bytes, more than three chunks"
    );
    assert!(
        read < total,
        "cancellation read the whole source, so the bound is the file and not a constant"
    );
}

/// T2.9 — a cancelled extraction removes its partial output
/// (FR-20, FR-18).
///
/// A truncated plaintext left on disk is indistinguishable from a short file,
/// which is exactly what HC-3 forbids.
#[test]
fn t2_9_a_cancelled_extraction_removes_its_partial_output() {
    let scratch = harness::Scratch::new("cancel-extract");
    let dir = scratch.vault_dir();
    let content = multi_chunk();

    let mut vault = create(&dir);
    let id = add(&mut vault, "big.bin", "d", &content);

    let destination = scratch.path("extracted.bin");
    let cancel = Cancel::new();
    let mut sink = CancelAt::new(cancel.clone(), CHUNK_LEN as u64);
    let outcome = vault.extract_to_path(id, &destination, &mut sink, &cancel);

    assert!(
        matches!(outcome, Err(Error::Cancelled { .. })),
        "expected a cancellation, got {outcome:?}"
    );
    assert!(
        !destination.exists(),
        "a partial extraction was left on disk"
    );

    // The entry itself is untouched, and extracts in full when asked again.
    let mut out = Vec::new();
    vault
        .extract(id, &mut out, &mut NoProgress, &Cancel::new())
        .unwrap();
    assert_eq!(out, content);
}
