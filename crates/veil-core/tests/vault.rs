//! Phase 1 test cases T1.26 through T1.32 — packs, extents, and the vertical
//! slice (HC-1, HC-2, HC-3, A-5, C-2, S-3, S-4, Spec §4.1–§4.5, §9).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use veil_core::Error;
use veil_core::crypto::{KdfParams, Password};
use veil_core::index::EntryId;
use veil_core::store::{entries_in_pack, existing_pack_ids, pack_path};
use veil_core::vault::Vault;
use veil_core::{Cancel, NoProgress};

/// Names and content of the shape HC-1 exists to protect.
const MARKER_NAME: &str = "exec_compensation_2024.csv";
const MARKER_FOLDER: &str = "HR/salaries";
const MARKER_CONTENT: &str = "SALARY-ROW-MARKER-9c1f";

/// Small enough that a multi-pack vault costs kilobytes rather than gigabytes.
/// The cap being a parameter is the reason this suite runs at all (P1.9.e).
const SMALL_CAP: u64 = 4096;

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

fn create(dir: &Path, cap: u64) -> Vault {
    Vault::create(dir, &password(), KdfParams::for_tests(), cap).unwrap()
}

fn read_back(vault: &Vault, id: EntryId) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    vault.extract(id, &mut out, &mut NoProgress, &Cancel::new())?;
    Ok(out)
}

/// T1.26 — an entry larger than the pack cap spans packs and reconstructs
/// exactly (C-2, A-2, Spec §4.5).
#[test]
fn t1_26_an_entry_spans_packs_and_reconstructs() {
    let scratch = Scratch::new("spanning");
    let mut vault = create(&scratch.vault_dir(), SMALL_CAP);

    let content = pattern(SMALL_CAP as usize * 5 + 123);
    let id = vault
        .add(
            "big.bin",
            "media",
            &mut content.as_slice(),
            &mut NoProgress,
            &Cancel::new(),
        )
        .unwrap();

    let entry = vault.entries().iter().find(|e| e.id == id).unwrap();
    assert!(
        entry.extents.len() > 1,
        "the entry did not span packs: {:?}",
        entry.extents
    );
    let packs: Vec<u32> = entry.extents.iter().map(|x| x.pack_id).collect();
    assert!(
        packs.windows(2).all(|w| w[1] == w[0] + 1),
        "packs not in order"
    );

    assert_eq!(read_back(&vault, id).unwrap(), content);
}

/// T1.27 — reading one entry touches no unrelated pack (A-5, Spec §4.5).
///
/// A-5 is the door held open for the mount deferral and the basis of the
/// product's first motivation: retrieving one file from a several-hundred-
/// gigabyte vault without touching the rest.
#[test]
fn t1_27_one_entry_reads_without_unrelated_packs() {
    let scratch = Scratch::new("locality");
    let mut vault = create(&scratch.vault_dir(), SMALL_CAP);

    let first = pattern(SMALL_CAP as usize * 2);
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

    let second_packs: Vec<u32> = vault
        .entries()
        .iter()
        .find(|e| e.id == second_id)
        .unwrap()
        .extents
        .iter()
        .map(|x| x.pack_id)
        .collect();

    // Destroy a pack the second entry has no extent in.
    let victim = vault
        .entries()
        .iter()
        .find(|e| e.id == first_id)
        .unwrap()
        .extents[0]
        .pack_id;
    assert!(!second_packs.contains(&victim), "the fixture overlapped");

    let path = pack_path(&scratch.vault_dir(), victim);
    let ruined = vec![0u8; std::fs::metadata(&path).unwrap().len() as usize];
    std::fs::write(&path, ruined).unwrap();

    assert_eq!(
        read_back(&vault, second_id).unwrap(),
        second,
        "an unrelated pack's damage prevented a read"
    );
}

/// T1.28 — pack damage is confined and attributed (S-4, HC-3, Spec §4.5) —
/// §9 corruption table, row 7.
///
/// **Naming the affected entries is half the requirement.** S-4 rejects two
/// failures at once: one bad region losing everything, and one bad region
/// being indistinguishable from total loss. A case that only asserts "some
/// entries failed" leaves the second half untested, and the second half is
/// what turns a partial failure into a list of files a user can restore from
/// a backup.
#[test]
fn t1_28_pack_damage_is_confined_and_named() {
    let scratch = Scratch::new("locality-attribution");
    let mut vault = create(&scratch.vault_dir(), SMALL_CAP);

    // Several entries spread over at least three packs.
    let mut ids = Vec::new();
    let mut contents = Vec::new();
    for i in 0..6u8 {
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

    let packs = existing_pack_ids(&scratch.vault_dir()).unwrap();
    assert!(
        packs.len() >= 3,
        "the fixture produced only {} packs",
        packs.len()
    );

    // Damage the middle pack.
    let victim = packs[packs.len() / 2];
    let path = pack_path(&scratch.vault_dir(), victim);
    let mut bytes = std::fs::read(&path).unwrap();
    for byte in &mut bytes {
        *byte ^= 0xFF;
    }
    std::fs::write(&path, &bytes).unwrap();

    // Attribution, computed from the extents alone.
    let named = entries_in_pack(vault.entries(), victim);
    assert!(
        !named.is_empty(),
        "no entry was attributed to the damaged pack"
    );

    // And observed behaviour matches the attribution exactly — not a superset,
    // not the first casualty, not "the vault".
    let mut observed_failures = Vec::new();
    for (id, content) in ids.iter().zip(&contents) {
        match read_back(&vault, *id) {
            Ok(out) => assert_eq!(&out, content, "{id} read back wrong bytes"),
            Err(Error::Corrupt { affected, .. }) => {
                assert_eq!(affected, vec![*id], "the failure named the wrong entry");
                observed_failures.push(*id);
            }
            Err(other) => panic!("{id} failed with {other:?} rather than damage"),
        }
    }

    observed_failures.sort();
    let mut expected = named;
    expected.sort();
    assert_eq!(
        observed_failures, expected,
        "the entries that failed are not the entries the extents name"
    );

    // Every other entry survives: one bad pack is not total loss.
    assert!(
        observed_failures.len() < ids.len(),
        "damaging one pack cost every entry"
    );
}

/// T1.29 — adding one entry dirties one pack and the index (S-3, Spec §4.5).
///
/// S-3's acceptance standard is that adding a small file to a large vault
/// causes a sync client to transfer megabytes rather than the vault. This is
/// that standard observed at the filesystem rather than inferred from the
/// format's design.
#[test]
fn t1_29_adding_an_entry_dirties_one_pack() {
    let scratch = Scratch::new("change-locality");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);

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

    let packs_changed: Vec<&String> = changed.iter().filter(|n| n.ends_with(".pack")).collect();
    assert_eq!(
        packs_changed.len(),
        1,
        "adding one small file changed {} packs: {changed:?}",
        packs_changed.len()
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

/// T1.30 — the vertical slice round-trips across a close and reopen
/// (HC-3, Spec §4.1–§4.5).
///
/// The first point at which header, key hierarchy, index persistence, packs,
/// and content encryption are proven to compose rather than to work
/// individually.
#[test]
fn t1_30_vertical_slice_round_trips_across_reopen() {
    let scratch = Scratch::new("slice");
    let dir = scratch.vault_dir();

    let content = pattern(9000);
    let id = {
        let mut vault = create(&dir, SMALL_CAP);
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

    // Closed before the next open. Since Phase 2 the vault holds an advisory
    // lock for its lifetime (FR-26), so a second open of a vault still held
    // here would report `VaultInUse` and this case would stop testing what it
    // is named for.
    drop(vault);

    // A wrong password on a real vault is still a wrong password.
    let wrong = Password::new("some other long password".to_owned());
    assert!(matches!(
        Vault::open(&dir, &wrong),
        Err(Error::WrongPassword)
    ));
}

/// T1.31 — nothing is written outside the vault directory (HC-2).
///
/// **Regression test.** The original Veil's extraction wrote into the current
/// working directory, which is how a truncated decryption came to overwrite
/// the user's own original. HC-2 states that no operation writes plaintext
/// anywhere the user has not designated, and the cost of asserting it is one
/// directory comparison.
#[test]
fn t1_31_nothing_is_written_outside_the_vault_directory() {
    let scratch = Scratch::new("containment");
    let dir = scratch.vault_dir();

    // The enclosing scratch directory stands in for the working directory: if
    // anything escapes the vault, it lands here.
    let before = snapshot(&scratch.0);

    let content = pattern(9000);
    let mut vault = create(&dir, SMALL_CAP);
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
                && !name.ends_with(".pack")
        })
        .collect();

    assert!(
        outside.is_empty(),
        "files appeared outside the vault's own: {outside:?}"
    );
}

/// T1.32 — a closed vault discloses nothing (HC-1).
///
/// **Regression test for the original's defining flaw**, at the level of the
/// whole vault rather than the index alone: `strings` over the original's
/// metadata database returned the full index with no password.
///
/// *Scope note:* HC-1 accepts that total size, component count and sizes, and
/// the fact that this is a Veil vault remain observable. This asserts the
/// prohibition, not the accepted disclosures.
#[test]
fn t1_32_a_closed_vault_discloses_nothing() {
    let scratch = Scratch::new("disclosure");
    let dir = scratch.vault_dir();

    {
        let mut vault = create(&dir, SMALL_CAP);
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

/// T1.28 — asking for an entry that does not exist is not damage.
///
/// This case asserted the opposite until Phase 3: the vault reported an unknown
/// identifier as damaged content, which is FR-2's conflation one level down —
/// a mistyped name and a corrupted vault send a user to different remedies.
#[test]
fn t1_28_an_unknown_entry_is_not_reported_as_damage() {
    let scratch = Scratch::new("unknown-entry");
    let vault = create(&scratch.vault_dir(), SMALL_CAP);

    match read_back(&vault, EntryId::new(999)) {
        Err(Error::NotFound) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}
