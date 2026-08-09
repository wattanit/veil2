//! Phase 5 test cases T5.11 through T5.13 — the portability fixture
//! (Spec §9; HC-8, FR-31, §4.6).
//!
//! Opens the vault committed at `tests/fixtures/portability/Fixture.veil`
//! and checks it against the manifest committed beside it. Built once by
//! `cargo run -p veil-core --example build_portability_fixture`; regenerated
//! only by hand, deliberately — see that file's header.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use serde::Deserialize;
use veil_core::crypto::Password;
use veil_core::vault::{Cancel, NoProgress, Vault};
use veil_core::{Error, Unrepresentable};

#[derive(Deserialize)]
struct ManifestEntry {
    folder: String,
    input_name: String,
    name: String,
    content: String,
    expect_refusal: Option<String>,
}

#[derive(Deserialize)]
struct Manifest {
    password: String,
    entries: Vec<ManifestEntry>,
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/portability")
}

fn manifest() -> Manifest {
    let bytes = std::fs::read(fixture_root().join("manifest.json"))
        .expect("the portability fixture's manifest is committed alongside it");
    serde_json::from_slice(&bytes).unwrap()
}

/// Serialises this file's tests against the one on-disk fixture.
///
/// The advisory lock (Spec §2) is exclusive regardless of read intent — by
/// design, matching Phase 2's "a second opener is told the vault is in use"
/// — and every test in this file opens the same path. Without this, `cargo
/// test`'s default parallelism turns every run but the first into
/// `VaultInUse`, which is a fact about test isolation, not about the fixture.
fn fixture_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn open_fixture() -> (Vault, Manifest, std::sync::MutexGuard<'static, ()>) {
    let guard = fixture_lock();
    let manifest = manifest();
    let vault = Vault::open(
        &fixture_root().join("Fixture.veil"),
        &Password::new(manifest.password.clone()),
    )
    .expect("the committed fixture must open with the password its own manifest records");
    (vault, manifest, guard)
}

/// T5.11 — The fixture opens and its manifest matches what it holds
/// (Spec §9, HC-8).
#[test]
fn t5_11_the_fixture_opens_and_its_manifest_matches_what_it_holds() {
    let (vault, manifest, _guard) = open_fixture();

    assert_eq!(
        vault.entries().len(),
        manifest.entries.len(),
        "the vault holds a different number of entries than the manifest records"
    );

    let mut actual: Vec<(&str, &str)> = vault
        .entries()
        .iter()
        .map(|e| (e.folder.as_str(), e.name.as_str()))
        .collect();
    actual.sort_unstable();

    let mut expected: Vec<(&str, &str)> = manifest
        .entries
        .iter()
        .map(|e| (e.folder.as_str(), e.name.as_str()))
        .collect();
    expected.sort_unstable();

    assert_eq!(
        actual, expected,
        "the fixture's entries drifted from its manifest"
    );
}

/// T5.12 — Every fixture entry extracts byte-identically (Spec §9, HC-8).
#[test]
fn t5_12_every_fixture_entry_extracts_byte_identically() {
    let (vault, manifest, _guard) = open_fixture();

    for entry in &manifest.entries {
        let stored = vault.find(&entry.folder, &entry.name).unwrap_or_else(|| {
            panic!(
                "{}/{} is in the manifest but not the vault",
                entry.folder, entry.name
            )
        });
        let mut out = Vec::new();
        vault
            .extract(stored.id, &mut out, &mut NoProgress, &Cancel::new())
            .unwrap();
        assert_eq!(
            out,
            entry.content.as_bytes(),
            "{}/{} did not extract byte-identically",
            entry.folder,
            entry.name
        );
    }

    // Every input was already NFC except the deliberately decomposed one —
    // confirming the manifest's own record of what was typed, not only what
    // ended up stored.
    for entry in &manifest.entries {
        if entry.folder == "nfd" {
            assert_ne!(
                entry.input_name, entry.name,
                "the NFD entry's input should differ from its stored form"
            );
        } else {
            assert_eq!(
                entry.input_name, entry.name,
                "{}/{} was normalised despite already being NFC",
                entry.folder, entry.name
            );
        }
    }

    // The NFC/NFD pair (P5.3.c): the folder holds exactly one entry, and it
    // is reachable by either spelling.
    let nfd_entries: Vec<_> = vault
        .entries()
        .iter()
        .filter(|e| e.folder == "nfd")
        .collect();
    assert_eq!(
        nfd_entries.len(),
        1,
        "the NFC/NFD pair must be one entry, not two"
    );
    let by_nfc = vault.find("nfd", "caf\u{e9}.txt").unwrap();
    let by_nfd = vault.find("nfd", "cafe\u{301}.txt").unwrap();
    assert_eq!(by_nfc.id, by_nfd.id);
}

/// T5.13 — The manifest states which reserved names refuse, and why
/// (Spec §4.6, FR-31).
#[test]
fn t5_13_the_manifest_states_which_names_refuse_and_why() {
    let (vault, manifest, _guard) = open_fixture();

    for entry in &manifest.entries {
        let stored = vault.find(&entry.folder, &entry.name).unwrap();
        let outcome = vault.check_representable(stored.id);

        match &entry.expect_refusal {
            None => assert!(
                outcome.is_ok(),
                "{}/{} was expected to be representable but was refused: {:?}",
                entry.folder,
                entry.name,
                outcome
            ),
            Some(reason) => match outcome {
                Err(Error::NameNotRepresentable { reason: actual, .. }) => assert_eq!(
                    format!("{actual:?}"),
                    *reason,
                    "{}/{} was refused for the wrong reason",
                    entry.folder,
                    entry.name
                ),
                other => panic!(
                    "{}/{} was expected to be refused as {reason} but got {other:?}",
                    entry.folder, entry.name
                ),
            },
        }
    }

    // A sanity check on the manifest itself: every `Unrepresentable` reason
    // this crate defines appears at least once, or the fixture is not
    // exercising the whole enum it claims to.
    let seen: std::collections::HashSet<String> = manifest
        .entries
        .iter()
        .filter_map(|e| e.expect_refusal.clone())
        .collect();
    for reason in [
        Unrepresentable::ReservedName,
        Unrepresentable::ReservedCharacter,
        Unrepresentable::TrailingDotOrSpace,
        Unrepresentable::CaseCollision,
    ] {
        assert!(
            seen.contains(&format!("{reason:?}")),
            "the fixture never exercises {reason:?}"
        );
    }
}
