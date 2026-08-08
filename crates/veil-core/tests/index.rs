//! Phase 1 test cases T1.20 through T1.25 — the index (HC-1, HC-3, HC-4,
//! FR-27, FR-30, Spec §4.3, §4.4).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use veil_core::crypto::{
    HASH_LEN, NONCE_PREFIX_LEN, WRAPPED_KEY_LEN, generate_master_key, index_key,
};
use veil_core::index::{Entry, EntryId, Extent, IndexDocument, Statistics, generations};
use veil_core::{Damaged, Error};

/// A distinctive name of the shape HC-1 exists to protect, taken from the
/// output `strings` produced against the original Veil.
const MARKER_NAME: &str = "exec_compensation_2024.csv";
const MARKER_FOLDER: &str = "HR/salaries";

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("veil2-index-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn marked_entry(id: u64) -> Entry {
    Entry {
        id: EntryId::new(id),
        name: MARKER_NAME.to_owned(),
        folder: MARKER_FOLDER.to_owned(),
        size: 4096,
        source_mtime: 1_700_000_000,
        added_at: 1_800_000_000,
        content_hash: [0xAB; HASH_LEN],
        wrapped_dek: [0xCD; WRAPPED_KEY_LEN],
        nonce_prefix: [0xEF; NONCE_PREFIX_LEN],
        extents: vec![Extent {
            pack_id: 1,
            offset: 0,
            length: 4112,
        }],
        unknown: BTreeMap::new(),
    }
}

fn document_with(entries: Vec<Entry>, generation: u64) -> IndexDocument {
    let mut doc = IndexDocument::empty();
    doc.generation = generation;
    doc.statistics = Statistics {
        entry_count: entries.len() as u64,
        logical_bytes: entries.iter().map(|e| e.size).sum(),
        physical_bytes: 0,
        reclaimable_bytes: 0,
    };
    doc.entries = entries;
    doc
}

/// T1.20 — a stored index discloses nothing (HC-1, Spec §4.3).
///
/// **This is the regression test for the original's defining flaw.** Running
/// `strings` over the original Veil's metadata database with no password
/// returned `vpath:/HR/salaries/exec_compensation_2024.csv` and the folder
/// keys around it. Motivation 2 of the Requirements — names are secrets —
/// exists because of that output.
#[test]
fn t1_20_a_stored_index_discloses_nothing() {
    let scratch = Scratch::new("disclosure");
    let master = generate_master_key();
    let key = index_key(&master);

    let doc = document_with(vec![marked_entry(1), marked_entry(2)], 1);

    // The search must be capable of finding the marker: the *plaintext* CBOR
    // contains it. Without this, a search that never matches anything would
    // pass against a vault that wrote the index in the clear.
    let plaintext = doc.to_cbor().unwrap();
    assert!(
        contains(&plaintext, MARKER_NAME.as_bytes()),
        "the search cannot find the marker even in plaintext; this test would \
         pass vacuously"
    );

    veil_core::index::write(scratch.path(), &key, &doc).unwrap();

    let mut checked = 0;
    for slot in ["index.a", "index.b"] {
        let path = scratch.path().join(slot);
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        checked += 1;

        for (label, needle) in encodings(MARKER_NAME)
            .into_iter()
            .chain(encodings(MARKER_FOLDER))
        {
            assert!(
                !contains(&bytes, &needle),
                "{slot} discloses {label} in the clear"
            );
        }
    }
    assert!(checked > 0, "no slot was written");
}

/// Every encoding a name could plausibly survive in.
///
/// Checking UTF-8 alone would pass a build that happened to store UTF-16, and
/// checking NFC alone would miss a name normalised the other way.
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

/// T1.21 — unknown fields survive a read and write cycle (FR-30, §4.3).
///
/// This is the reader's half of the migration door that Requirements §2.2
/// defers and HC-5 and FR-30 hold open. A reader that drops what it does not
/// understand turns a future migration into a reconstruction.
#[test]
fn t1_21_unknown_fields_are_preserved() {
    let mut entry = marked_entry(1);
    entry.unknown.insert(
        "future_entry_field".to_owned(),
        ciborium::Value::Text("kept".to_owned()),
    );

    let mut doc = document_with(vec![entry], 3);
    doc.unknown.insert(
        "future_document_field".to_owned(),
        ciborium::Value::Integer(4242.into()),
    );

    // Encode, decode, mutate something unrelated, re-encode, decode again.
    let once = IndexDocument::from_cbor(&doc.to_cbor().unwrap()).unwrap();
    let mut mutated = once.clone();
    mutated.generation = 4;
    let twice = IndexDocument::from_cbor(&mutated.to_cbor().unwrap()).unwrap();

    assert_eq!(
        twice.unknown.get("future_document_field"),
        Some(&ciborium::Value::Integer(4242.into())),
        "a document-level unknown field was dropped"
    );
    assert_eq!(
        twice.entries[0].unknown.get("future_entry_field"),
        Some(&ciborium::Value::Text("kept".to_owned())),
        "an entry-level unknown field was dropped"
    );
    assert_eq!(twice.generation, 4, "the known field did not round-trip");
}

/// T1.22 — writes alternate slots and the newest authenticating generation
/// wins (HC-4, §4.4).
#[test]
fn t1_22_writes_alternate_and_the_newest_wins() {
    let scratch = Scratch::new("alternate");
    let master = generate_master_key();
    let key = index_key(&master);

    for generation in 1..=6u64 {
        let doc = document_with(vec![marked_entry(generation)], generation);
        veil_core::index::write(scratch.path(), &key, &doc).unwrap();

        let read = veil_core::index::read(scratch.path(), &key).unwrap();
        assert_eq!(read.generation, generation, "read did not take the newest");
        assert_eq!(read.entries[0].id, EntryId::new(generation));
    }

    // Both slots are in use, and they hold adjacent generations: a write went
    // to the older slot every time rather than always to the same one.
    let [a, b] = generations(scratch.path());
    let (a, b) = (a.expect("slot a"), b.expect("slot b"));
    assert_eq!(a.abs_diff(b), 1, "writes did not alternate: {a} and {b}");
}

/// T1.23 — a damaged newer slot falls back to the previous generation (HC-4).
///
/// **This is where "the older slot is expendable" is cashed in.** The
/// Specification chose two slots over a rename because slot expendability
/// holds on every platform while rename atomicity does not. That reasoning is
/// only worth something if this path runs before a real crash runs it.
#[test]
fn t1_23_a_damaged_newer_slot_falls_back() {
    for (label, damage) in [
        ("tag", usize::MAX),
        ("body", 40usize),
        ("preamble generation", 5),
        ("magic", 0),
    ] {
        let scratch = Scratch::new(&format!("fallback-{}", label.replace(' ', "-")));
        let master = generate_master_key();
        let key = index_key(&master);

        veil_core::index::write(
            scratch.path(),
            &key,
            &document_with(vec![marked_entry(1)], 1),
        )
        .unwrap();
        veil_core::index::write(
            scratch.path(),
            &key,
            &document_with(vec![marked_entry(2)], 2),
        )
        .unwrap();

        // Damage whichever slot holds the higher generation.
        let [gen_a, gen_b] = generations(scratch.path());
        let newer = if gen_a > gen_b { "index.a" } else { "index.b" };
        let path = scratch.path().join(newer);
        let mut bytes = std::fs::read(&path).unwrap();
        let at = if damage == usize::MAX {
            bytes.len() - 1
        } else {
            damage
        };
        bytes[at] ^= 0xFF;
        std::fs::write(&path, &bytes).unwrap();

        let read = veil_core::index::read(scratch.path(), &key)
            .unwrap_or_else(|e| panic!("{label}: the vault did not open at all: {e}"));
        assert_eq!(
            read.generation, 1,
            "{label}: fell through to something other than the previous generation"
        );
        assert_eq!(read.entries[0].id, EntryId::new(1));
    }
}

/// T1.24 — both slots unusable is a loud failure (HC-3, HC-4).
///
/// Never an empty index, never a partially recovered one, never a guess.
#[test]
fn t1_24_both_slots_damaged_fails_loudly() {
    let scratch = Scratch::new("both-damaged");
    let master = generate_master_key();
    let key = index_key(&master);

    veil_core::index::write(
        scratch.path(),
        &key,
        &document_with(vec![marked_entry(1)], 1),
    )
    .unwrap();
    veil_core::index::write(
        scratch.path(),
        &key,
        &document_with(vec![marked_entry(2)], 2),
    )
    .unwrap();

    for slot in ["index.a", "index.b"] {
        let path = scratch.path().join(slot);
        let mut bytes = std::fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        std::fs::write(&path, &bytes).unwrap();
    }

    match veil_core::index::read(scratch.path(), &key) {
        Err(Error::Corrupt {
            what: Damaged::BothIndexSlots,
            ..
        }) => {}
        other => panic!("expected both-slots damage, got {other:?}"),
    }
}

/// T1.24 — a wrong index key is a failure, not an empty index.
#[test]
fn t1_24_a_wrong_key_does_not_yield_an_empty_index() {
    let scratch = Scratch::new("wrong-key");
    let key = index_key(&generate_master_key());
    let other = index_key(&generate_master_key());

    veil_core::index::write(
        scratch.path(),
        &key,
        &document_with(vec![marked_entry(1)], 1),
    )
    .unwrap();

    assert!(
        veil_core::index::read(scratch.path(), &other).is_err(),
        "an index decrypted under the wrong key"
    );
}

/// T1.25 — the generation counter advances by one and never repeats
/// (FR-27, §4.4).
///
/// FR-27's detection of external modification is built on this counter, so a
/// skipped or reused generation is a defect in the detector rather than a
/// cosmetic issue.
#[test]
fn t1_25_generations_are_strictly_increasing() {
    let scratch = Scratch::new("generations");
    let master = generate_master_key();
    let key = index_key(&master);

    let mut seen = Vec::new();
    for generation in 1..=8u64 {
        veil_core::index::write(
            scratch.path(),
            &key,
            &document_with(vec![marked_entry(generation)], generation),
        )
        .unwrap();
        seen.push(
            veil_core::index::read(scratch.path(), &key)
                .unwrap()
                .generation,
        );
    }

    for pair in seen.windows(2) {
        assert_eq!(
            pair[1],
            pair[0] + 1,
            "generations are not strictly increasing by one: {seen:?}"
        );
    }
}

/// T1.24 — a slot whose preamble disagrees with its document is refused.
///
/// The preamble is authenticated, so this cannot happen through tampering —
/// it would take a writer bug. A slot whose two generations disagree is not
/// trustworthy either way, and taking the preamble's word for it is how a
/// stale index gets presented as current.
#[test]
fn t1_24_a_slot_with_inconsistent_generations_is_refused() {
    let scratch = Scratch::new("inconsistent");
    let master = generate_master_key();
    let key = index_key(&master);

    veil_core::index::write(
        scratch.path(),
        &key,
        &document_with(vec![marked_entry(1)], 1),
    )
    .unwrap();

    // Rewrite the preamble's generation without touching the ciphertext.
    let [gen_a, _] = generations(scratch.path());
    let slot = if gen_a.is_some() {
        "index.a"
    } else {
        "index.b"
    };
    let path = scratch.path().join(slot);
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[4..12].copy_from_slice(&99u64.to_le_bytes());
    std::fs::write(&path, &bytes).unwrap();

    assert!(
        veil_core::index::read(scratch.path(), &key).is_err(),
        "a slot claiming a generation its document does not carry was accepted"
    );
}
