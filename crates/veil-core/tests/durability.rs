//! Phase 4 test cases T4.1, T4.17 to T4.24 and T4.26 to T4.28 — write ordering,
//! reconciliation at open, and a pack that is gone
//! (Spec §4.5, §4.7; FR-32, HC-4, S-4).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use std::path::Path;

use harness::{
    SMALL_CAP, add, assert_statistics_match_recount, create, open, pattern, read_back, snapshot,
};
use veil_core::store::{PACKS_DIR, existing_pack_ids, pack_path};
use veil_core::vault::{Access, Cancel, NoProgress, Reconciled};

/// A pack that this entry uses and no other does.
///
/// Packs are shared: a sink appends to the newest one until it is full, so the
/// pack an entry starts in usually holds the tail of the one before. A case
/// about losing *one* entry's storage has to name a pack no other entry has a
/// stake in, or it is a case about losing two.
fn exclusive_pack(vault: &veil_core::vault::Vault, id: veil_core::index::EntryId) -> u32 {
    let mine: Vec<u32> = vault
        .entries()
        .iter()
        .find(|e| e.id == id)
        .unwrap()
        .extents
        .iter()
        .map(|x| x.pack_id)
        .collect();
    let shared: Vec<u32> = vault
        .entries()
        .iter()
        .filter(|e| e.id != id)
        .flat_map(|e| e.extents.iter())
        .map(|x| x.pack_id)
        .collect();
    *mine
        .iter()
        .find(|p| !shared.contains(p))
        .expect("the fixture gave this entry no pack of its own")
}

/// Content big enough to span several packs at [`SMALL_CAP`], so that some of
/// them belong to one entry alone.
fn spanning(seed: usize) -> Vec<u8> {
    pattern(3 * SMALL_CAP as usize + seed)
}

/// Writes a pack file that no entry references — the residue a crash leaves.
fn plant_orphan(dir: &Path, bytes: usize) -> (u32, u64) {
    let id = existing_pack_ids(dir).unwrap().last().map_or(1, |n| n + 1);
    let path = pack_path(dir, id);
    std::fs::create_dir_all(dir.join(PACKS_DIR)).unwrap();
    std::fs::write(&path, pattern(bytes)).unwrap();
    (id, bytes as u64)
}

// ---------------------------------------------------------------- ordering --

/// T4.1 — every write path is a known write path (FR-12, HC-4).
///
/// A tripwire, not a proof: what proves the ordering is killing a process
/// (T4.2 to T4.8). This exists because a durability obligation is the kind that
/// is met once and then quietly broken by a change that had no idea it was
/// participating. A new call site fails this case, and the fix is to review it
/// and add it here — never to widen the check.
#[test]
fn t4_1_every_write_path_is_a_known_write_path() {
    /// `(file, how many names it may create or remove)`. Every one of these
    /// has been checked against §4.7's ordering and, where it changes a
    /// directory, against P4.1.b's obligation to sync that directory.
    const KNOWN: [(&str, usize); 5] = [
        // The staging header, and the rename over the live one. Both synced,
        // and the vault directory synced after the rename.
        ("src/vault/session.rs", 2),
        // One slot file per index commit. Synced, and the directory too the
        // first time each slot appears.
        ("src/index/slots.rs", 1),
        // A pack created by the sink, and three removals: two undoing an
        // abandoned write, one in `remove_pack`. The packs directory is synced
        // after each.
        ("src/store/pack.rs", 4),
        // Extraction creates the destination it owns and removes it when the
        // content fails to authenticate (FR-17). Outside the vault, so the
        // vault's durability is not what is at stake.
        ("src/vault/read.rs", 2),
        // The lock file, created once at open. Holds nothing that must survive
        // a crash — the lock lives in the operating system, not in the file.
        ("src/vault/lock.rs", 1),
    ];
    const CALLS: [&str; 4] = ["File::create", "remove_file", "fs::rename", "create(true)"];

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut unreviewed = Vec::new();

    let mut stack = vec![root.join("src")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let source = std::fs::read_to_string(&path).unwrap();
            let found: usize = CALLS
                .iter()
                .map(|call| source.matches(call).count())
                .sum::<usize>();

            let allowed = KNOWN
                .iter()
                .find(|(name, _)| *name == relative)
                .map_or(0, |(_, n)| *n);
            if found != allowed {
                unreviewed.push(format!("{relative}: {found} where {allowed} were reviewed"));
            }
        }
    }

    assert!(
        unreviewed.is_empty(),
        "the set of write paths changed and each one owes a directory sync (§4.7, HC-4):\n  {}",
        unreviewed.join("\n  ")
    );
}

// ---------------------------------------------------------- reconciliation --

/// T4.17 — residue is found at open and reported, not destroyed (FR-32, HC-4).
///
/// FR-32 asks for it to be discarded here. It is not, and that is the decision
/// T4.28 exists to justify: whether unreferenced bytes are residue is a guess,
/// and acting destructively on a guess is how a vault mid-sync loses data that
/// no interruption touched. What FR-32 requires and gets is the report — space
/// that reappears without explanation is indistinguishable, to the person
/// watching, from space that was never accounted for properly.
#[test]
fn t4_17_residue_is_found_and_reported() {
    let scratch = harness::Scratch::new("reconcile-orphan");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);
    let id = add(&mut vault, "kept.bin", "d", &pattern(1500));
    let before = vault.statistics();
    drop(vault);

    let (orphan, size) = plant_orphan(&dir, 2048);

    let vault = open(&dir).unwrap();
    assert_eq!(vault.reconciled(), Reconciled::Residue { bytes: size });
    assert!(
        pack_path(&dir, orphan).exists(),
        "the residue was destroyed at open rather than reported"
    );
    // Counted into what the user can reclaim, so nothing accumulates unseen.
    assert_eq!(
        vault.statistics().reclaimable_bytes,
        before.reclaimable_bytes + size
    );
    assert_eq!(read_back(&vault, id).unwrap(), pattern(1500));
    assert_statistics_match_recount(&vault, "after residue was found");

    // And reclaiming space takes it, which is the deliberate act FR-23 wants.
    let mut vault = vault;
    let reclaimed = vault.compact(&mut NoProgress, &Cancel::new()).unwrap();
    assert!(reclaimed.bytes_recovered >= size);
    assert!(!pack_path(&dir, orphan).exists());
    assert_eq!(read_back(&vault, id).unwrap(), pattern(1500));
}

/// T4.18 — an open never writes (HC-4, S-2, FR-27).
///
/// An open that writes is an open that can fail, and every read of a vault
/// would become a durability event. It would also cost FR-27 its detector: an
/// index write advances the generation, so a vault opened from a stale copy
/// would come away holding a number higher than the newer index a sync daemon
/// then delivers, and every later write would sail past the check meant to
/// refuse it.
#[test]
fn t4_18_an_open_never_writes() {
    let scratch = harness::Scratch::new("reconcile-quiet");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);
    add(&mut vault, "one.bin", "d", &pattern(900));
    let generation = vault.generation();
    drop(vault);

    let before = snapshot(&dir);
    let vault = open(&dir).unwrap();
    assert_eq!(vault.reconciled(), Reconciled::Clean);
    assert_eq!(vault.generation(), generation, "the generation advanced");
    drop(vault);
    assert_eq!(snapshot(&dir), before, "an ordinary open changed the vault");

    // And with residue present, which is the case that tempts a write.
    plant_orphan(&dir, 1024);
    let before = snapshot(&dir);
    let vault = open(&dir).unwrap();
    assert!(matches!(vault.reconciled(), Reconciled::Residue { .. }));
    assert_eq!(vault.generation(), generation);
    drop(vault);
    assert_eq!(
        snapshot(&dir),
        before,
        "an open that found residue wrote to the vault"
    );
}

/// T4.19 — an interrupted reclaim leaves nothing unreachable (FR-32, HC-4,
/// FR-24).
///
/// Both sides of the commit boundary, reached without a seam: the pre-commit
/// side is a new pack nothing references, the post-commit side is an old pack
/// nothing references any more. Either way every live file still reads, the
/// leftover is reported as reclaimable, and running the operation again
/// finishes the job.
#[test]
fn t4_19_an_interrupted_reclaim_leaves_nothing_unreachable() {
    let scratch = harness::Scratch::new("reconcile-reclaim");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);

    let mut kept = Vec::new();
    for n in 0..4 {
        let content = pattern(3000 + n);
        kept.push((
            add(&mut vault, &format!("f{n}.bin"), "d", &content),
            content,
        ));
    }
    vault.delete(kept.remove(0).0).unwrap();

    // The pre-commit side: a fresh pack written and abandoned, exactly what a
    // kill between `finish` and the index commit leaves behind.
    drop(vault);
    let (leftover, size) = plant_orphan(&dir, 3072);

    let vault = open(&dir).unwrap();
    assert_eq!(vault.reconciled().residue_bytes(), size);
    assert!(pack_path(&dir, leftover).exists());
    for (id, content) in &kept {
        assert_eq!(&read_back(&vault, *id).unwrap(), content);
    }
    assert_statistics_match_recount(&vault, "after an interrupted reclaim");

    // Reclaiming takes the leftover along with the rest, and everything live
    // survives the operation that was interrupted the first time.
    //
    // Asserted on bytes rather than on the leftover's file still existing:
    // removing the highest pack frees its number, and the pack this reclaim
    // writes may take it back. Recorded as *Notes for Upstream*, item 9.
    let mut vault = vault;
    let on_disk_before = veil_core::store::total_pack_bytes(&dir).unwrap();
    let reclaimed = vault.compact(&mut NoProgress, &Cancel::new()).unwrap();
    assert!(reclaimed.bytes_recovered >= size);
    assert!(veil_core::store::total_pack_bytes(&dir).unwrap() < on_disk_before);
    assert_eq!(vault.statistics().reclaimable_bytes, 0);
    drop(vault);

    let vault = open(&dir).unwrap();
    assert_eq!(vault.reconciled(), Reconciled::Clean);
    for (id, content) in &kept {
        assert_eq!(&read_back(&vault, *id).unwrap(), content);
    }
}

/// T4.20 — a read-only vault opens, skips reconciliation, and says so
/// (FR-32, Spec §4.5, §4.8).
///
/// Refusing to open would turn an interrupted reclaim on a drive that later
/// became read-only into permanent data loss, which HC-4 forbids.
#[test]
#[cfg(unix)]
fn t4_20_a_read_only_vault_skips_reconciliation() {
    use std::os::unix::fs::PermissionsExt;

    let scratch = harness::Scratch::new("reconcile-readonly");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);
    let id = add(&mut vault, "kept.bin", "d", &pattern(1200));
    drop(vault);

    let (orphan, _) = plant_orphan(&dir, 1024);

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
        eprintln!("T4.20 skipped: this account can write regardless of permissions");
        return;
    }

    let vault = open(&dir).unwrap();
    assert_eq!(vault.access(), Access::ReadOnly);
    assert_eq!(vault.reconciled(), Reconciled::Skipped);
    assert_eq!(vault.reconciled().residue_bytes(), 0);
    assert!(
        pack_path(&dir, orphan).exists(),
        "something was written to a read-only vault"
    );
    // Reading works, which is the whole point of opening it at all.
    assert_eq!(read_back(&vault, id).unwrap(), pattern(1200));
    assert!(vault.verify(&mut NoProgress, &Cancel::new()).is_ok());

    drop(vault);
    restore(&dir, &denied);
}

/// Gives the permissions back, so the scratch directory can be removed.
#[cfg(unix)]
fn restore(dir: &Path, files: &[std::path::PathBuf]) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
    for path in files {
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644));
    }
}

/// T4.21 — garbage in the middle of a live pack is left alone (FR-23, FR-32).
///
/// Reconciliation discards what can go without moving a byte. Bytes with live
/// bytes *after* them cannot: recovering those means rewriting the pack around
/// them, which is reclaiming space, and FR-23 makes that the user's decision
/// alone. So the file this deletes is the one at the *start* of the pack — the
/// case where the hole it leaves has something live above it.
#[test]
fn t4_21_garbage_inside_a_live_pack_is_left_alone() {
    let scratch = harness::Scratch::new("reconcile-live-pack");
    let dir = scratch.vault_dir();
    // A cap big enough that both files share one pack.
    let mut vault = create(&dir, 1024 * 1024);
    let doomed = add(&mut vault, "doomed.bin", "d", &pattern(700));
    add(&mut vault, "kept.bin", "d", &pattern(800));
    vault.delete(doomed).unwrap();
    let reclaimable = vault.statistics().reclaimable_bytes;
    assert!(reclaimable > 0);
    drop(vault);

    let packs = existing_pack_ids(&dir).unwrap();
    let sizes: Vec<u64> = packs
        .iter()
        .map(|id| std::fs::metadata(pack_path(&dir, *id)).unwrap().len())
        .collect();

    let vault = open(&dir).unwrap();
    assert_eq!(vault.reconciled().residue_bytes(), 0);
    assert_eq!(vault.statistics().reclaimable_bytes, reclaimable);
    assert_eq!(existing_pack_ids(&dir).unwrap(), packs);
    let now: Vec<u64> = packs
        .iter()
        .map(|id| std::fs::metadata(pack_path(&dir, *id)).unwrap().len())
        .collect();
    assert_eq!(now, sizes, "a live pack was rewritten by an open");
}

/// T4.26 — deleted bytes are not residue and are never taken at open
/// (FR-21, FR-23, FR-29, FR-32).
///
/// The other side of T4.17. Both are bytes on disk that no entry references,
/// and they call for opposite treatment: residue goes, a deleted file's bytes
/// stay until the user asks for the space back. The discriminator is already in
/// the index — the statistics count what committed operations put on disk, the
/// filesystem counts what is there, and the difference is exactly the residue.
///
/// Asserted in the shape that would break a rule based on "unreferenced" alone:
/// a pack the deletes emptied completely, and a dead tail with nothing live
/// above it. Both are trivially discardable and neither may be discarded.
#[test]
fn t4_26_deleted_bytes_are_not_residue() {
    // A whole pack that nothing references any more.
    let scratch = harness::Scratch::new("reconcile-emptied");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);

    let doomed = add(&mut vault, "doomed.bin", "d", &spanning(0));
    let kept = add(&mut vault, "kept.bin", "d", &spanning(7));
    let emptied = exclusive_pack(&vault, doomed);

    vault.delete(doomed).unwrap();
    let before = vault.statistics();
    drop(vault);

    let vault = open(&dir).unwrap();
    assert_eq!(vault.reconciled().residue_bytes(), 0);
    assert!(
        pack_path(&dir, emptied).exists(),
        "a pack the user emptied by deleting was taken at open"
    );
    assert_eq!(vault.statistics(), before, "the figures moved on their own");
    assert!(vault.statistics().reclaimable_bytes > 0);
    assert_eq!(read_back(&vault, kept).unwrap(), spanning(7));
    drop(vault);

    // And a dead tail: the last file in a pack that still holds others. Nothing
    // live sits above it, so a rule that cut whatever it could would cut here.
    let scratch = harness::Scratch::new("reconcile-tail");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, 1024 * 1024);
    let first = add(&mut vault, "first.bin", "d", &pattern(900));
    let last = add(&mut vault, "last.bin", "d", &pattern(1100));
    let pack = vault
        .entries()
        .iter()
        .find(|e| e.id == last)
        .unwrap()
        .extents[0]
        .pack_id;
    let size_before = std::fs::metadata(pack_path(&dir, pack)).unwrap().len();

    vault.delete(last).unwrap();
    let reclaimable = vault.statistics().reclaimable_bytes;
    drop(vault);

    let vault = open(&dir).unwrap();
    assert_eq!(vault.reconciled().residue_bytes(), 0);
    assert_eq!(
        std::fs::metadata(pack_path(&dir, pack)).unwrap().len(),
        size_before,
        "a dead tail the user may still want to reclaim was cut at open"
    );
    assert_eq!(vault.statistics().reclaimable_bytes, reclaimable);
    assert_eq!(read_back(&vault, first).unwrap(), pattern(900));
    assert_statistics_match_recount(&vault, "after an open that took nothing");
}

/// T4.27 — the residue of an interrupted ingest is found, tail and all
/// (FR-32, HC-4).
///
/// The case T4.26 is the mirror of. Bytes appended by an operation that never
/// committed are counted by nothing, so the difference between what the
/// statistics claim and what the filesystem holds finds them exactly — even
/// when they sit as a tail on a live pack, which is where an interrupted add
/// leaves them and where a rule about whole packs would miss them entirely.
#[test]
fn t4_27_the_residue_of_an_interrupted_ingest_is_found() {
    let scratch = harness::Scratch::new("reconcile-residue");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, 1024 * 1024);
    let kept = add(&mut vault, "kept.bin", "d", &pattern(900));
    let pack = vault
        .entries()
        .iter()
        .find(|e| e.id == kept)
        .unwrap()
        .extents[0]
        .pack_id;
    let before = vault.statistics();
    drop(vault);

    // Bytes appended to a live pack that no commit ever learned of — exactly
    // what a kill part-way through an add leaves behind.
    let path = pack_path(&dir, pack);
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    std::io::Write::write_all(&mut file, &pattern(5000)).unwrap();
    drop(file);

    let vault = open(&dir).unwrap();
    assert_eq!(vault.reconciled(), Reconciled::Residue { bytes: 5000 });
    assert_eq!(
        vault.statistics().reclaimable_bytes,
        before.reclaimable_bytes + 5000,
        "the residue was not counted into what can be reclaimed"
    );
    assert_eq!(read_back(&vault, kept).unwrap(), pattern(900));
    assert_statistics_match_recount(&vault, "after residue was found");

    // Reclaiming takes it, and the file that shared the pack is untouched.
    let mut vault = vault;
    let reclaimed = vault.compact(&mut NoProgress, &Cancel::new()).unwrap();
    assert_eq!(reclaimed.bytes_recovered, 5000);
    assert_eq!(read_back(&vault, kept).unwrap(), pattern(900));
    assert_eq!(vault.statistics().reclaimable_bytes, 0);
}

/// T4.28 — a vault whose index is behind its packs loses nothing (HC-4, FR-27).
///
/// The case that decides why residue is reported rather than destroyed. A sync
/// daemon replicating a vault can deliver an older index before the packs a
/// newer one describes; opened at that moment, the vault sees bytes its index
/// does not account for, and they are indistinguishable from the residue of a
/// killed ingest. Discarding them would destroy content the newer index —
/// arriving seconds later — still points at, with no interruption anywhere in
/// the story. HC-4 does not allow that, so the guess is never acted on.
#[test]
fn t4_28_an_index_behind_its_packs_loses_nothing() {
    let scratch = harness::Scratch::new("reconcile-stale-index");
    let dir = scratch.vault_dir();

    let mut vault = create(&dir, SMALL_CAP);
    add(&mut vault, "first.bin", "d", &pattern(900));
    let older = snapshot(&dir);
    let arriving = add(&mut vault, "second.bin", "d", &pattern(1100));
    let newer = snapshot(&dir);
    drop(vault);

    // The daemon has delivered the packs but not yet the index that names them.
    for (name, bytes) in &older {
        if name.starts_with("index.") {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
    }

    let stale = open(&dir).unwrap();
    assert_eq!(stale.entries().len(), 1);
    assert!(matches!(stale.reconciled(), Reconciled::Residue { .. }));
    drop(stale);

    // Now the newer index lands. What it points at has to still be there.
    for (name, bytes) in &newer {
        if name.starts_with("index.") {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
    }

    let vault = open(&dir).unwrap();
    assert_eq!(vault.entries().len(), 2);
    assert_eq!(
        read_back(&vault, arriving).unwrap(),
        pattern(1100),
        "the file the newer index names was destroyed by an earlier open"
    );
    assert_eq!(vault.reconciled(), Reconciled::Clean);
    assert_statistics_match_recount(&vault, "after the newer index arrived");
}

// ------------------------------------------------------------ missing pack --

/// T4.22 — a missing pack opens the vault and names its casualties
/// (S-4, Spec §4.5).
///
/// Refusing to open would convert the loss of one pack into the loss of the
/// whole vault, which is the failure S-4 exists to reject.
#[test]
fn t4_22_a_missing_pack_opens_and_names_its_casualties() {
    let scratch = harness::Scratch::new("missing-pack");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);

    let lost = add(&mut vault, "lost.bin", "d", &spanning(0));
    add(&mut vault, "safe.bin", "d", &spanning(9));
    let victim = exclusive_pack(&vault, lost);
    drop(vault);

    std::fs::remove_file(pack_path(&dir, victim)).unwrap();

    let vault = open(&dir).unwrap();
    assert_eq!(vault.missing_packs(), vec![victim]);
    assert_eq!(vault.unreadable_entries(), vec![lost]);
    // Enumerated without reading anything: the entry is still listed.
    assert_eq!(vault.entries().len(), 2);
}

/// T4.23 — everything outside the missing pack still works (S-4).
///
/// S-4 is not the claim that damage is detected; it is the claim that damage is
/// bounded and attributable, and only the second half is worth anything to
/// someone deciding whether to reach for a backup.
#[test]
fn t4_23_everything_outside_a_missing_pack_still_works() {
    let scratch = harness::Scratch::new("missing-pack-rest");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);

    let lost = add(&mut vault, "lost.bin", "d", &spanning(0));
    let safe = add(&mut vault, "safe.bin", "d", &spanning(11));
    let victim = exclusive_pack(&vault, lost);
    drop(vault);
    std::fs::remove_file(pack_path(&dir, victim)).unwrap();

    let vault = open(&dir).unwrap();
    assert_eq!(read_back(&vault, safe).unwrap(), spanning(11));
    assert!(read_back(&vault, lost).is_err());

    let report = vault.verify(&mut NoProgress, &Cancel::new()).unwrap();
    let failed: Vec<_> = report
        .verdicts
        .iter()
        .filter(|v| !matches!(v.outcome, veil_core::vault::Outcome::Passed))
        .map(|v| v.id)
        .collect();
    assert_eq!(
        failed,
        vec![lost],
        "damage was not confined to the one file"
    );
}

/// T4.24 — a missing pack is never treated as garbage (FR-32, S-4).
///
/// A missing pack is referenced by definition, so it is damage and not residue.
/// An implementation that confuses the two deletes the record of what the user
/// lost.
#[test]
fn t4_24_a_missing_pack_is_never_treated_as_garbage() {
    let scratch = harness::Scratch::new("missing-not-garbage");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);

    let lost = add(&mut vault, "lost.bin", "d", &spanning(0));
    add(&mut vault, "safe.bin", "d", &spanning(13));
    let victim = exclusive_pack(&vault, lost);
    let before = vault.statistics();
    let generation = vault.generation();
    drop(vault);
    std::fs::remove_file(pack_path(&dir, victim)).unwrap();

    let vault = open(&dir).unwrap();
    assert_eq!(vault.reconciled().residue_bytes(), 0);
    assert_eq!(vault.entries().len(), 2, "an entry was dropped");
    assert_eq!(
        vault.generation(),
        generation,
        "damage was written into the index"
    );
    assert_eq!(
        vault.statistics(),
        before,
        "the figures were adjusted to match the damage rather than reporting it"
    );
}
