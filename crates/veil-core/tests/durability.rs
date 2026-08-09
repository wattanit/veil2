//! Phase 4 test cases T4.1, T4.8 to T4.12 — write ordering, residue, a
//! missing entry file, and read-only media (Spec §4.5, §4.7; HC-4, S-3).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use std::path::Path;

use harness::{add, create, open, pattern, read_back, snapshot};
use veil_core::index::EntryId;
use veil_core::vault::{Access, Cancel, NoProgress, Outcome};

/// Writes an entry file that no entry in the index references — the residue
/// an interrupted replace or delete leaves behind (Spec §4.5).
fn plant_residue(dir: &Path, id: u64, bytes: usize) -> EntryId {
    let id = EntryId::new(id);
    std::fs::create_dir_all(dir.join(veil_core::store::ENTRIES_DIR)).unwrap();
    std::fs::write(veil_core::store::entry_path(dir, id), pattern(bytes)).unwrap();
    id
}

// ---------------------------------------------------------------- ordering --

/// T4.1 — every write path is a known write path (FR-12, HC-4).
///
/// A tripwire, not a proof: what proves the ordering is killing a process
/// (`veil-cli`'s `tests/crashes.rs`). This exists because a durability
/// obligation is the kind that is met once and then quietly broken by a
/// change that had no idea it was participating. A new call site fails this
/// case, and the fix is to review it and add it here — never to widen the
/// check.
#[test]
fn t4_1_every_write_path_is_a_known_write_path() {
    /// `(file, how many names it may create or remove)`. Every one of these
    /// has been checked against §4.7's ordering and, where it changes a
    /// directory, against the obligation to sync that directory too.
    const KNOWN: [(&str, usize); 5] = [
        // The staging header, and the rename over the live one. Both synced,
        // and the vault directory synced after the rename.
        ("src/vault/session.rs", 2),
        // One slot file per index commit. Synced, and the directory too the
        // first time each slot appears.
        ("src/index/slots.rs", 1),
        // One entry file created by `EntryWriter::create`, and one removal in
        // `store::remove` (delete and replace both call it). The entries
        // directory is synced after each.
        ("src/store/entry_file.rs", 2),
        // Extraction creates the destination it owns and removes it when the
        // content fails to authenticate (FR-18). Outside the vault, so the
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

// ------------------------------------------------------------------ residue --

/// T4.8 — residue from an interrupted operation is left alone (HC-4,
/// Spec §4.5).
///
/// Nothing looks for it at open, because opening a vault walks no entry files
/// and writes nothing. Nothing removes it later either: there is no reclaim
/// mechanism to hand it to, by decision — an index that is momentarily behind
/// its own directory is indistinguishable from this case by construction, and
/// removing a file on that guess risks the loss HC-4 forbids.
#[test]
fn t4_8_residue_from_an_interrupted_operation_is_left_alone() {
    let scratch = harness::Scratch::new("residue");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir);
    let id = add(&mut vault, "kept.bin", "d", &pattern(1500));
    let before = vault.statistics();
    drop(vault);

    let residue = plant_residue(&dir, 999, 2048);

    let vault = open(&dir).unwrap();
    assert!(
        veil_core::store::exists(&dir, residue),
        "the residue was destroyed at open rather than left alone"
    );
    assert_eq!(
        vault.statistics(),
        before,
        "opening the vault changed the figures it reports"
    );
    assert_eq!(
        vault.entries().len(),
        1,
        "the residue was adopted as an entry rather than left unreferenced"
    );
    assert_eq!(read_back(&vault, id).unwrap(), pattern(1500));

    // Checking and listing do not touch it either.
    let report = vault.verify(&mut NoProgress, &Cancel::new()).unwrap();
    assert!(report.all_passed());
    drop(vault);
    assert!(
        veil_core::store::exists(&dir, residue),
        "something removed the residue without being asked"
    );
}

/// T4.11 — an open never writes (HC-4, S-2, FR-24).
///
/// An open that writes is an open that can fail, and every read of a vault
/// would become a durability event. It would also cost FR-24 its detector: an
/// index write advances the generation, so a vault opened from a stale copy
/// would come away holding a number higher than the newer index a sync daemon
/// then delivers, and every later write would sail past the check meant to
/// refuse it.
#[test]
fn t4_11_an_open_never_writes() {
    let scratch = harness::Scratch::new("open-writes-nothing");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir);
    add(&mut vault, "one.bin", "d", &pattern(900));
    let generation = vault.generation();
    drop(vault);

    let before = snapshot(&dir);
    let vault = open(&dir).unwrap();
    assert_eq!(vault.generation(), generation, "the generation advanced");
    drop(vault);
    assert_eq!(snapshot(&dir), before, "an ordinary open changed the vault");

    // And with residue present, which is the case that tempts a write.
    plant_residue(&dir, 999, 1024);
    let before = snapshot(&dir);
    let vault = open(&dir).unwrap();
    assert_eq!(vault.generation(), generation);
    drop(vault);
    assert_eq!(
        snapshot(&dir),
        before,
        "an open with residue present wrote to the vault"
    );
}

// ------------------------------------------------------------ missing file --

/// T4.9 — a missing entry file opens the vault and names its casualty
/// (S-3, Spec §4.5).
///
/// Refusing to open would convert the loss of one entry into the loss of the
/// whole vault, which is the failure S-3 exists to reject. With one file per
/// entry there is no attribution to compute — the missing file already names
/// the entry it belongs to.
#[test]
fn t4_9_a_missing_entry_file_opens_the_vault_and_names_its_casualty() {
    let scratch = harness::Scratch::new("missing-entry");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir);

    let lost = add(&mut vault, "lost.bin", "d", &pattern(700));
    add(&mut vault, "safe.bin", "d", &pattern(900));
    drop(vault);

    std::fs::remove_file(veil_core::store::entry_path(&dir, lost)).unwrap();

    let vault = open(&dir).unwrap();
    assert_eq!(vault.unreadable_entries(), vec![lost]);
    // Enumerated without reading anything: the entry is still listed.
    assert_eq!(vault.entries().len(), 2);
}

/// T4.10 — everything outside the missing entry still works (S-3).
///
/// S-3 is not the claim that damage is detected; it is the claim that damage
/// is bounded and attributable, and only the second half is worth anything to
/// someone deciding whether to reach for a backup.
#[test]
fn t4_10_everything_outside_the_missing_entry_still_works() {
    let scratch = harness::Scratch::new("missing-entry-rest");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir);

    let lost = add(&mut vault, "lost.bin", "d", &pattern(700));
    let safe = add(&mut vault, "safe.bin", "d", &pattern(900));
    drop(vault);
    std::fs::remove_file(veil_core::store::entry_path(&dir, lost)).unwrap();

    let vault = open(&dir).unwrap();
    assert_eq!(read_back(&vault, safe).unwrap(), pattern(900));
    assert!(read_back(&vault, lost).is_err());

    let report = vault.verify(&mut NoProgress, &Cancel::new()).unwrap();
    let failed: Vec<_> = report
        .verdicts
        .iter()
        .filter(|v| !matches!(v.outcome, Outcome::Passed))
        .map(|v| v.id)
        .collect();
    assert_eq!(
        failed,
        vec![lost],
        "damage was not confined to the one file"
    );
}

// -------------------------------------------------------------- read-only --

/// T4.12 — a read-only vault opens, says so, and is not written to (FR-23,
/// Spec §4.5, §4.8).
///
/// Refusing to open would turn an interrupted operation on a drive that later
/// became read-only into permanent data loss, which HC-4 forbids.
#[test]
#[cfg(unix)]
fn t4_12_a_read_only_vault_opens_and_says_so() {
    use std::os::unix::fs::PermissionsExt;

    let scratch = harness::Scratch::new("readonly");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir);
    let id = add(&mut vault, "kept.bin", "d", &pattern(1200));
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
        eprintln!("T4.12 skipped: this account can write regardless of permissions");
        return;
    }

    let vault = open(&dir).unwrap();
    assert_eq!(vault.access(), Access::ReadOnly);
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
