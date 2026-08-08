//! Phase 2 test cases T2.35 and T2.36 — property tests (Spec §9; FR-16,
//! FR-22).
//!
//! `proptest` searches; T2.16 and T2.26 fix one case each. Both are wanted: a
//! fixed case names the operation that broke, and a search finds the case
//! nobody thought to fix.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use proptest::prelude::*;

use harness::{SMALL_CAP, assert_statistics_match_recount, create, pattern};
use veil_core::crypto::CHUNK_LEN;
use veil_core::{Cancel, NoProgress};

/// Lengths that include zero, one, and the chunk boundary and its neighbours.
///
/// **Generated deliberately, not hoped for.** A uniform generator over a
/// megabyte reaches the exact chunk length essentially never, and that is where
/// a one-chunk-lookahead implementation breaks.
fn interesting_length() -> impl Strategy<Value = usize> {
    prop_oneof![
        4 => 0usize..4096,
        1 => Just(0usize),
        1 => Just(1usize),
        1 => Just(CHUNK_LEN - 1),
        1 => Just(CHUNK_LEN),
        1 => Just(CHUNK_LEN + 1),
        1 => (CHUNK_LEN - 8)..(CHUNK_LEN + 8),
    ]
}

/// One mutation in a generated sequence.
#[derive(Debug, Clone)]
enum Step {
    Add(usize),
    Replace(usize, usize),
    Delete(usize),
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        3 => (0usize..6000).prop_map(Step::Add),
        2 => (0usize..8, 0usize..6000).prop_map(|(which, len)| Step::Replace(which, len)),
        1 => (0usize..8).prop_map(Step::Delete),
    ]
}

proptest! {
    // Content generation dominates the cost here, so the case count is chosen
    // for a suite that runs on every push rather than for the default.
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// T2.35 — any byte sequence at any length survives a round trip
    /// (Spec §9, FR-16).
    #[test]
    fn t2_35_any_content_survives_a_round_trip(
        lengths in prop::collection::vec(interesting_length(), 1..4)
    ) {
        let scratch = harness::Scratch::new(&format!("prop-roundtrip-{}", lengths.len()));
        let dir = scratch.vault_dir();
        let mut vault = create(&dir, SMALL_CAP);

        for (index, length) in lengths.iter().enumerate() {
            let content = pattern(*length);
            let id = vault
                .add(
                    &format!("f{index}.bin"),
                    "d",
                    &mut content.as_slice(),
                    &mut NoProgress,
                    &Cancel::new(),
                )
                .unwrap();
            let read = harness::read_back(&vault, id).unwrap();
            prop_assert_eq!(read.len(), content.len());
            prop_assert!(read == content, "length {} did not round trip", length);
        }

        // And the whole vault still verifies, so the entries did not damage
        // each other on the way in.
        prop_assert!(vault.verify(&mut NoProgress, &Cancel::new()).unwrap().all_passed());
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// T2.36 — any sequence of operations keeps statistics true
    /// (FR-22, Spec §9).
    ///
    /// T2.26 fixes one sequence; this searches for the one that diverges.
    #[test]
    fn t2_36_any_sequence_of_operations_keeps_statistics_true(
        steps in prop::collection::vec(step(), 1..14)
    ) {
        let scratch = harness::Scratch::new(&format!("prop-stats-{}", steps.len()));
        let dir = scratch.vault_dir();
        let mut vault = create(&dir, SMALL_CAP);
        assert_statistics_match_recount(&vault, "empty");

        let mut counter = 0usize;
        for (index, step) in steps.iter().enumerate() {
            match step {
                Step::Add(length) => {
                    counter += 1;
                    let name = format!("f{counter}.bin");
                    vault
                        .add(
                            &name,
                            "d",
                            &mut pattern(*length).as_slice(),
                            &mut NoProgress,
                            &Cancel::new(),
                        )
                        .unwrap();
                }
                Step::Replace(which, length) => {
                    // A path that is not present is a refusal, and a refusal
                    // must leave the totals alone just as a success must keep
                    // them true.
                    let target = vault
                        .entries()
                        .get(which % vault.entries().len().max(1))
                        .map(|e| (e.folder.clone(), e.name.clone()));
                    if let Some((folder, name)) = target {
                        vault
                            .replace(
                                &folder,
                                &name,
                                &mut pattern(*length).as_slice(),
                                &mut NoProgress,
                                &Cancel::new(),
                            )
                            .unwrap();
                    }
                }
                Step::Delete(which) => {
                    let target = vault
                        .entries()
                        .get(which % vault.entries().len().max(1))
                        .map(|e| e.id);
                    if let Some(id) = target {
                        vault.delete(id).unwrap();
                    }
                }
            }
            assert_statistics_match_recount(&vault, &format!("after step {index}: {step:?}"));
        }

        // The figures survive a close and reopen: they live in the index, not
        // in the process.
        //
        // **Amended in Phase 4.** Open now reconciles (FR-32), and a sequence
        // of deletes and replaces can leave a pack with nothing live in it —
        // stored data no entry references, so it goes, and the totals fall by
        // exactly what it held. The property this case is about is that the
        // totals stay *true*, which the recount below asserts directly and
        // which no longer implies they are unchanged by an open.
        let before = vault.statistics();
        drop(vault);
        let vault = harness::open(&dir).unwrap();
        let recovered = vault.reconciled().bytes_recovered();

        prop_assert_eq!(vault.statistics().entry_count, before.entry_count);
        prop_assert_eq!(vault.statistics().logical_bytes, before.logical_bytes);
        prop_assert_eq!(
            vault.statistics().physical_bytes,
            before.physical_bytes - recovered
        );
        prop_assert_eq!(
            vault.statistics().reclaimable_bytes,
            before.reclaimable_bytes - recovered
        );
        assert_statistics_match_recount(&vault, "after a reopen");
    }
}
