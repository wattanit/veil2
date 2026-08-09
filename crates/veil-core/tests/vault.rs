//! Phase 1 test cases T1.26 through T1.31 — entry files and the vertical
//! slice (HC-1, HC-2, HC-3, A-5, S-3, Spec §4.1–§4.5, §9).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use veil_core::Error;
use veil_core::crypto::{KdfParams, Password};
use veil_core::index::EntryId;
use veil_core::vault::Vault;
use veil_core::{Cancel, NoProgress};

/// Names and content of the shape HC-1 exists to protect.
const MARKER_NAME: &str = "exec_compensation_2024.csv";
const MARKER_FOLDER: &str = "HR/salaries";
const MARKER_CONTENT: &str = "SALARY-ROW-MARKER-9c1f";

fn password() -> Password {
    Password::new("a sufficiently long password".to_owned())
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("veil2-vault-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn vault_dir(&self) -> PathBuf {
        self.0.join("Test.veil")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

fn create(dir: &Path) -> Vault {
    Vault::create(dir, &password(), KdfParams::for_tests()).unwrap()
}

fn read_back(vault: &Vault, id: EntryId) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    vault.extract(id, &mut out, &mut NoProgress, &Cancel::new())?;
    Ok(out)
}

/// T1.26 — reading one entry touches no other entry's file (A-5, Spec §4.1).
///
/// A-5 is the door held open for the mount deferral and the basis of the
/// product's first motivation: retrieving one file from a several-hundred-
/// gigabyte vault without touching the rest. Under one-file-per-entry
/// storage this is structural — there is no shared container to seek within
/// — and this case makes it observable rather than only argued.
#[test]
fn t1_26_reading_one_entry_touches_no_other_entry_file() {
    let scratch = Scratch::new("locality");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir);

    let first = pattern(2000);
    let second = pattern(700);
    let first_id = vault
        .add(
            "first.bin",
            "a",
            &mut first.as_slice(),
            &mut NoProgress,
            &Cancel::new(),
        )
        .unwrap();
    let second_id = vault
        .add(
            "second.bin",
            "b",
            &mut second.as_slice(),
            &mut NoProgress,
            &Cancel::new(),
        )
        .unwrap();

    // Destroy the file backing an entry other than the one being read.
    let victim = veil_core::store::entry_path(&dir, first_id);
    let ruined = vec![0u8; std::fs::metadata(&victim).unwrap().len() as usize];
    std::fs::write(&victim, ruined).unwrap();

    assert_eq!(
        read_back(&vault, second_id).unwrap(),
        second,
        "an unrelated entry's damaged file prevented a read"
    );
}

/// T1.27 — a corrupted entry file fails only that entry (S-3, HC-3, Spec
/// §4.5) — §9 corruption table, row 7.
///
/// **Naming the affected entry is half the requirement.** S-3 rejects two
/// failures at once: one bad file losing everything, and one bad file being
/// indistinguishable from total loss. With one file per entry, damage cannot
/// spread past its own file — there is no pack-level attribution to compute,
/// only the confinement itself.
#[test]
fn t1_27_a_corrupted_entry_file_fails_only_that_entry() {
    let scratch = Scratch::new("confinement");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir);

    let mut ids = Vec::new();
    let mut contents = Vec::new();
    for i in 0..4u8 {
        let content = pattern(1500 + usize::from(i) * 200);
        let id = vault
            .add(
                &format!("file{i}.bin"),
                "docs",
                &mut content.as_slice(),
                &mut NoProgress,
                &Cancel::new(),
            )
            .unwrap();
        ids.push(id);
        contents.push(content);
    }

    let victim = ids[1];
    let path = veil_core::store::entry_path(&dir, victim);
    let mut bytes = std::fs::read(&path).unwrap();
    for byte in &mut bytes {
        *byte ^= 0xFF;
    }
    std::fs::write(&path, &bytes).unwrap();

    for (id, content) in ids.iter().zip(&contents) {
        match read_back(&vault, *id) {
            Ok(out) if *id != victim => assert_eq!(&out, content, "{id} read back wrong bytes"),
            Ok(_) => panic!("the damaged entry read back successfully"),
            Err(Error::Corrupt { affected, .. }) if *id == victim => {
                assert_eq!(affected, vec![victim], "the failure named the wrong entry");
            }
            Err(other) => panic!("{id} failed with {other:?} rather than damage"),
        }
    }
}

/// T1.28 — adding one entry writes exactly one new file (S-3, Spec §4.5).
///
/// No other entry's file is touched, and the change to the vault is bounded
/// by the size of what was added, not the size of what was already there.
#[test]
fn t1_28_adding_one_entry_writes_exactly_one_file() {
    let scratch = Scratch::new("change-locality");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir);

    for i in 0..5u8 {
        let content = pattern(3000 + usize::from(i));
        vault
            .add(
                &format!("file{i}.bin"),
                "docs",
                &mut content.as_slice(),
                &mut NoProgress,
                &Cancel::new(),
            )
            .unwrap();
    }

    let before = snapshot(&dir);
    let small = pattern(64);
    vault
        .add(
            "tiny.txt",
            "docs",
            &mut small.as_slice(),
            &mut NoProgress,
            &Cancel::new(),
        )
        .unwrap();
    let after = snapshot(&dir);

    let changed: Vec<String> = after
        .iter()
        .filter(|(name, digest)| before.iter().all(|(n, d)| n != name || d != digest))
        .map(|(name, _)| name.clone())
        .collect();

    let entry_files_changed: Vec<&String> =
        changed.iter().filter(|n| n.ends_with(".entry")).collect();
    assert_eq!(
        entry_files_changed.len(),
        1,
        "adding one small file changed {} entry files: {changed:?}",
        entry_files_changed.len()
    );
    assert!(
        changed.iter().any(|n| n.starts_with("index.")),
        "the index was not updated: {changed:?}"
    );
    assert!(
        !changed.iter().any(|n| n == "veil.header"),
        "the header was rewritten for an ordinary add: {changed:?}"
    );
}

/// Every file in the vault, by name, with a digest of its contents.
fn snapshot(dir: &Path) -> Vec<(String, [u8; 32])> {
    let mut out = Vec::new();
    let mut walk = vec![dir.to_path_buf()];
    while let Some(current) = walk.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk.push(path);
            } else if let Ok(bytes) = std::fs::read(&path) {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_owned();
                out.push((name, *blake3::hash(&bytes).as_bytes()));
            }
        }
    }
    out.sort();
    out
}

/// T1.29 — the vertical slice round-trips across a close and reopen (HC-3,
/// Spec §4.1–§4.5).
///
/// The first point at which header, key hierarchy, index persistence,
/// entry-file storage, and content encryption are proven to compose rather
/// than to work individually.
#[test]
fn t1_29_vertical_slice_round_trips_across_reopen() {
    let scratch = Scratch::new("slice");
    let dir = scratch.vault_dir();

    let content = pattern(9000);
    let id = {
        let mut vault = create(&dir);
        let id = vault
            .add(
                MARKER_NAME,
                MARKER_FOLDER,
                &mut content.as_slice(),
                &mut NoProgress,
                &Cancel::new(),
            )
            .unwrap();
        assert_eq!(read_back(&vault, id).unwrap(), content);
        id
    };

    // Everything in memory is gone; only the directory remains.
    let vault = Vault::open(&dir, &password()).expect("the vault reopens");
    assert_eq!(vault.entries().len(), 1);
    let entry = &vault.entries()[0];
    assert_eq!(entry.name, MARKER_NAME);
    assert_eq!(entry.folder, MARKER_FOLDER);
    assert_eq!(entry.size, content.len() as u64);
    assert_eq!(entry.unknown, BTreeMap::new());
    assert_eq!(read_back(&vault, id).unwrap(), content);

    // Closed before the next open: the vault holds an advisory lock for its
    // lifetime (FR-23), so a second open of a vault still held here would
    // report `VaultInUse` and this case would stop testing what it is named
    // for.
    drop(vault);

    // A wrong password on a real vault is still a wrong password.
    let wrong = Password::new("some other long password".to_owned());
    assert!(matches!(
        Vault::open(&dir, &wrong),
        Err(Error::WrongPassword)
    ));
}

/// T1.30 — nothing is written outside the vault directory (HC-2).
///
/// **Regression test.** The original Veil's extraction wrote into the current
/// working directory, which is how a truncated decryption came to overwrite
/// the user's own original. HC-2 states that no operation writes plaintext
/// anywhere the user has not designated, and the cost of asserting it is one
/// directory comparison.
#[test]
fn t1_30_nothing_is_written_outside_the_vault_directory() {
    let scratch = Scratch::new("containment");
    let dir = scratch.vault_dir();

    // The enclosing scratch directory stands in for the working directory: if
    // anything escapes the vault, it lands here.
    let before = snapshot(&scratch.0);

    let content = pattern(9000);
    let mut vault = create(&dir);
    let id = vault
        .add(
            MARKER_NAME,
            MARKER_FOLDER,
            &mut content.as_slice(),
            &mut NoProgress,
            &Cancel::new(),
        )
        .unwrap();
    let mut sink = Vec::new();
    vault
        .extract(id, &mut sink, &mut NoProgress, &Cancel::new())
        .unwrap();
    drop(vault);

    let after = snapshot(&scratch.0);
    let outside: Vec<&(String, [u8; 32])> = after
        .iter()
        .filter(|item| !before.contains(item))
        .filter(|(name, _)| {
            // Everything the vault legitimately owns lives under its own
            // directory; these are its file names.
            name != "veil.header"
                && name != "veil.lock"
                && !name.starts_with("index.")
                && !name.ends_with(".entry")
        })
        .collect();

    assert!(
        outside.is_empty(),
        "files appeared outside the vault's own: {outside:?}"
    );
}

/// T1.31 — a closed vault discloses nothing (HC-1).
///
/// **Regression test for the original's defining flaw**, at the level of the
/// whole vault rather than the index alone: `strings` over the original's
/// metadata database returned the full index with no password.
///
/// *Scope note:* HC-1 accepts that total size, component count and sizes, and
/// the fact that this is a Veil vault remain observable. This asserts the
/// prohibition, not the accepted disclosures.
#[test]
fn t1_31_a_closed_vault_discloses_nothing() {
    let scratch = Scratch::new("disclosure");
    let dir = scratch.vault_dir();

    {
        let mut vault = create(&dir);
        let mut content = MARKER_CONTENT.as_bytes().to_vec();
        content.extend_from_slice(&pattern(4000));
        vault
            .add(
                MARKER_NAME,
                MARKER_FOLDER,
                &mut content.as_slice(),
                &mut NoProgress,
                &Cancel::new(),
            )
            .unwrap();
    }

    let mut files = 0;
    for (path, bytes) in every_file(&dir) {
        files += 1;
        for (label, needle) in encodings(MARKER_NAME)
            .into_iter()
            .chain(encodings(MARKER_FOLDER))
            .chain(encodings(MARKER_CONTENT))
        {
            assert!(
                !contains(&bytes, &needle),
                "{} discloses {label}",
                path.display()
            );
        }
    }
    assert!(files >= 3, "only {files} files were searched");
}

fn every_file(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut walk = vec![dir.to_path_buf()];
    while let Some(current) = walk.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk.push(path);
            } else if let Ok(bytes) = std::fs::read(&path) {
                out.push((path, bytes));
            }
        }
    }
    out
}

fn encodings(text: &str) -> Vec<(String, Vec<u8>)> {
    let utf16: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
    vec![
        (format!("{text} (UTF-8)"), text.as_bytes().to_vec()),
        (format!("{text} (UTF-16LE)"), utf16),
    ]
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Asking for an entry that does not exist is not damage.
///
/// This is FR-2's conflation one level down: a mistyped name and a corrupted
/// vault send a user to different remedies. Not tied to the storage-layer
/// rewrite this file otherwise covers; kept here because it exercises the
/// same lifecycle as the cases above.
#[test]
fn unknown_entry_is_not_reported_as_damage() {
    let scratch = Scratch::new("unknown-entry");
    let vault = create(&scratch.vault_dir());

    match read_back(&vault, EntryId::new(999)) {
        Err(Error::NotFound) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}
