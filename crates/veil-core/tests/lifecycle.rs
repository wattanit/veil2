//! Phase 2 test cases T2.1 through T2.5 — lifecycle and locking
//! (FR-3, FR-6, FR-22, FR-26, FR-27, FR-33, S-2, A-7, Spec §2, §4.3, §4.4).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use harness::{SMALL_CAP, add, create, open, password, pattern};
use veil_core::vault::{Access, LOCK_FILE, Vault};
use veil_core::{Cancel, Error, NoProgress};

/// T2.1 — a second opener is told the vault is in use (FR-26, Spec §2, §6).
///
/// Not a corruption error and not an I/O error: a user whose vault is open in
/// another window must be told that, not sent to look for damage.
#[test]
fn t2_1_a_second_opener_is_told_the_vault_is_in_use() {
    let scratch = harness::Scratch::new("in-use");
    let dir = scratch.vault_dir();

    let held = create(&dir, SMALL_CAP);

    let second = Vault::open(&dir, &password());
    assert!(
        matches!(second, Err(Error::VaultInUse)),
        "expected VaultInUse, got {:?}",
        second.err()
    );

    // Creating over a held vault is the same conflict arriving by another
    // route, and answering it differently would be two remedies for one
    // condition.
    let recreate = Vault::create(
        &dir,
        &password(),
        veil_core::crypto::KdfParams::for_tests(),
        SMALL_CAP,
    );
    assert!(matches!(recreate, Err(Error::VaultInUse)));

    drop(held);
    // Once released it opens normally: the refusal was about contention, not
    // about the vault.
    assert!(open(&dir).is_ok());
}

/// T2.2 — the lock does not outlive the vault, including on failure
/// (FR-26, HC-4).
///
/// A leaked lock reports a user's own vault as in use, and the remedy is a file
/// they were never told about.
#[test]
fn t2_2_the_lock_does_not_outlive_the_vault() {
    let scratch = harness::Scratch::new("lock-release");
    let dir = scratch.vault_dir();

    // An ordinary close.
    let vault = create(&dir, SMALL_CAP);
    vault.lock();
    let mut vault = open(&dir).expect("reopens after an ordinary close");

    // An operation that fails partway, then a drop.
    let missing = veil_core::EntryId::new(9_999);
    assert!(vault.delete(missing).is_err());
    assert!(
        vault
            .extract(missing, &mut Vec::new(), &mut NoProgress, &Cancel::new())
            .is_err()
    );
    drop(vault);
    assert!(open(&dir).is_ok(), "reopens after a failed operation");

    // An unwind. Release is `Drop`'s job, which is the whole reason it is
    // `Drop` and not a call at the end of a successful path.
    let dir_for_panic = dir.clone();
    let panicked = std::panic::catch_unwind(move || {
        let _vault = Vault::open(&dir_for_panic, &password()).unwrap();
        panic!("deliberate");
    });
    assert!(panicked.is_err());
    assert!(open(&dir).is_ok(), "reopens after an unwind");

    // The lock file itself stays. It is the lock that is released, not the
    // file, and a leftover file must never be mistaken for a held lock — that
    // mistake is what makes stale lock files a support burden.
    assert!(dir.join(LOCK_FILE).exists());
}

/// T2.3 — locking a vault consumes it (FR-3, HC-2, Spec §5.1).
///
/// The assertion available from outside is the structural one: `lock` takes
/// `self`, so a locked vault is not reachable as a value with a flag set.
/// Zeroisation is asserted by T0.5 and T0.6 against the key types, which is
/// where the memory is.
#[test]
fn t2_3_locking_a_vault_consumes_it() {
    let scratch = harness::Scratch::new("lock-consumes");
    let dir = scratch.vault_dir();

    let mut vault = create(&dir, SMALL_CAP);
    add(&mut vault, "a.bin", "f", &pattern(100));
    vault.lock();

    // The proof available from outside: the value is gone, and the vault is
    // openable again, so the lock went with it.
    assert!(open(&dir).is_ok());
}

/// T2.4 — open reads the index and nothing else (FR-6, FR-22, FR-33, S-2, A-7).
///
/// **Asserted by removing the packs, not by timing.** A timing assertion on
/// shared CI hardware is a flake generator. What S-2 states is that vault size
/// is not an input to the work done at open; a vault whose pack files are gone
/// entirely still opening, enumerating every entry, and reporting its
/// statistics is that property in its strongest form — nothing that reads a
/// pack could survive it. It is simultaneously the FR-33 assertion: if
/// verification ran at open, this could not pass.
#[test]
fn t2_4_open_reads_the_index_and_nothing_else() {
    let scratch = harness::Scratch::new("open-cost");
    let dir = scratch.vault_dir();

    let mut vault = create(&dir, SMALL_CAP);
    for i in 0..8 {
        add(&mut vault, &format!("f{i}.bin"), "d", &pattern(2000));
    }
    let expected = vault.statistics();
    vault.lock();

    let packs = veil_core::store::existing_pack_ids(&dir).unwrap();
    assert!(
        packs.len() > 1,
        "the vault must span packs for this case to mean anything"
    );
    for id in &packs {
        std::fs::remove_file(veil_core::store::pack_path(&dir, *id)).unwrap();
    }

    let vault = open(&dir).expect("a vault opens without its packs");
    assert_eq!(vault.entries().len(), 8);
    assert_eq!(vault.statistics(), expected);
    for entry in vault.entries() {
        assert!(entry.size > 0);
    }
    drop(vault);

    // Two vaults open at once in one process (A-7). Nothing about an open vault
    // is process-global, which is what keeps the single-vault limit a product
    // decision rather than a structural one.
    let other_dir = scratch.path("Second.veil");
    create(&other_dir, SMALL_CAP).lock();

    let a = open(&dir).unwrap();
    let b = open(&other_dir).unwrap();
    assert_eq!(a.entries().len(), 8);
    assert_eq!(b.entries().len(), 0);
    assert_eq!(a.access(), Access::ReadWrite);
    assert_eq!(b.access(), Access::ReadWrite);
}

/// T2.5 — a vault changed on disk since open is not written over
/// (FR-27, Spec §4.3, §4.4).
///
/// **The external writer is a file copy, because that is what it is in life.**
/// Vaults live in sync folders (§1, motivation 3), so the change arrives as a
/// daemon replacing index slot files underneath an open vault, not as a second
/// Veil process — which is also why the advisory lock does not see it, and why
/// §2's honesty clause names the generation counter as the actual protection.
#[test]
fn t2_5_a_vault_changed_on_disk_is_not_written_over() {
    let scratch = harness::Scratch::new("changed-on-disk");
    let dir = scratch.vault_dir();

    let mut vault = create(&dir, SMALL_CAP);
    add(&mut vault, "original.bin", "d", &pattern(200));

    // The state a sync daemon would have replicated before the outside write.
    let older = harness::snapshot(&dir);

    let outside_id = add(&mut vault, "outside.bin", "d", &pattern(300));
    let newer = harness::snapshot(&dir);
    drop(vault);

    // Roll the directory back, so an opener sees the older generation…
    restore_slots(&dir, &older);
    let mut stale = open(&dir).unwrap();
    assert_eq!(stale.entries().len(), 1);

    // …then let the daemon deliver the newer generation underneath it.
    restore_slots(&dir, &newer);

    let refused = stale.add(
        "late.bin",
        "d",
        &mut pattern(50).as_slice(),
        &mut NoProgress,
        &Cancel::new(),
    );
    assert!(
        matches!(refused, Err(Error::ChangedOnDisk)),
        "expected ChangedOnDisk, got {:?}",
        refused.err()
    );

    // Every other write path passes through the same check, or FR-27 would
    // hold for `add` and quietly not for the rest.
    assert!(matches!(
        stale.delete(veil_core::EntryId::new(1)),
        Err(Error::ChangedOnDisk)
    ));
    assert!(matches!(
        stale.replace(
            "d",
            "original.bin",
            &mut pattern(10).as_slice(),
            &mut NoProgress,
            &Cancel::new(),
        ),
        Err(Error::ChangedOnDisk)
    ));
    drop(stale);

    // The outside write survives. Refusing is only correct if what it protects
    // is still there.
    let after = open(&dir).unwrap();
    assert!(after.entries().iter().any(|e| e.id == outside_id));
    assert!(after.entries().iter().all(|e| e.name != "late.bin"));
}

/// T2.41 — a changed vault reloads without the password (FR-27).
///
/// Detecting the change and refusing to write over it is only half of FR-27.
/// Requiring the password again to get past it would make "offer to reload" a
/// re-open in disguise, and would mean the safe answer costs more than the
/// unsafe one.
#[test]
fn t2_41_a_changed_vault_reloads_without_the_password() {
    let scratch = harness::Scratch::new("reload");
    let dir = scratch.vault_dir();

    let mut vault = create(&dir, SMALL_CAP);
    add(&mut vault, "original.bin", "d", &pattern(200));
    let older = harness::snapshot(&dir);
    let outside_id = add(&mut vault, "outside.bin", "d", &pattern(300));
    let newer = harness::snapshot(&dir);
    drop(vault);

    restore_slots(&dir, &older);
    let mut stale = open(&dir).unwrap();
    assert_eq!(stale.entries().len(), 1);
    restore_slots(&dir, &newer);

    assert!(matches!(
        stale.add(
            "late.bin",
            "d",
            &mut pattern(50).as_slice(),
            &mut NoProgress,
            &Cancel::new()
        ),
        Err(Error::ChangedOnDisk)
    ));

    // Reload, then the same write succeeds — on top of the outside change
    // rather than over it.
    stale.reload().unwrap();
    assert_eq!(stale.entries().len(), 2);
    assert!(stale.entries().iter().any(|e| e.id == outside_id));

    stale
        .add(
            "late.bin",
            "d",
            &mut pattern(50).as_slice(),
            &mut NoProgress,
            &Cancel::new(),
        )
        .expect("the write succeeds once the change has been adopted");
    assert_eq!(stale.entries().len(), 3);
    harness::assert_statistics_match_recount(&stale, "after a reload and a write");

    // The outside entry is still readable, so the reload adopted its content
    // and not only its bookkeeping.
    assert_eq!(
        harness::read_back(&stale, outside_id).unwrap(),
        pattern(300)
    );
}

/// Puts the index slots back to a recorded state, as a sync daemon would.
fn restore_slots(dir: &std::path::Path, state: &std::collections::BTreeMap<String, Vec<u8>>) {
    for (name, bytes) in state {
        if name.starts_with("index.") {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
    }
}
