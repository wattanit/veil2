//! Builds the Phase 5 portability fixture and commits it as bytes
//! (Plan P5.3; Spec §9, HC-8).
//!
//! Run once, by hand, when the fixture needs to change:
//! `cargo run -p veil-core --example build_portability_fixture`
//!
//! Not a test, and not run by the suite. The suite opens what this writes
//! (`tests/portability_fixture.rs`); regenerating it is a deliberate,
//! reviewed act — the whole point of a committed fixture is that a platform
//! years from now opens *these exact bytes*, not bytes freshly minted by
//! whatever wrote them this time.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::path::PathBuf;

use serde::Serialize;
use veil_core::crypto::{KdfParams, Password};
use veil_core::vault::{Cancel, NoProgress, Vault};

/// Long enough for C-4, and not a secret — a committed fixture's password is
/// public the moment it is committed, so it lives in the manifest rather
/// than being duplicated by hand between this file and the test that reads
/// it.
const FIXTURE_PASSWORD: &str = "the portability fixture password";

/// One entry this fixture stores.
struct Entry {
    folder: &'static str,
    /// What is actually given to `Vault::add` — the point at which P5.1
    /// normalises it.
    input_name: &'static str,
    content: &'static str,
    /// The `Unrepresentable` reason's variant name if extraction should
    /// refuse this entry (matching `veil_core::Unrepresentable`'s `Debug`
    /// output), or `None` if it should succeed.
    expect_refusal: Option<&'static str>,
}

/// "café.txt", decomposed (`e` + a combining acute accent) — the input;
/// P5.1 stores it precomposed. Kept as a `const` because Rust source cannot
/// spell an NFD sequence unambiguously without one.
const NFD_CAFE: &str = "cafe\u{301}.txt";

const ENTRIES: &[Entry] = &[
    // One script per family HC-8 names explicitly (Spec §9), each a plain,
    // representable name.
    Entry {
        folder: "latin",
        input_name: "report.pdf",
        content: "Latin content.",
        expect_refusal: None,
    },
    Entry {
        folder: "\u{0e20}\u{0e32}\u{0e29}\u{0e32}\u{0e44}\u{0e17}\u{0e22}", // ภาษาไทย
        input_name: "\u{0e23}\u{0e32}\u{0e22}\u{0e07}\u{0e32}\u{0e19}.txt", // รายงาน.txt
        content: "\u{0e40}\u{0e19}\u{0e37}\u{0e49}\u{0e2d}\u{0e2b}\u{0e32}\u{0e20}\u{0e32}\u{0e29}\u{0e32}\u{0e44}\u{0e17}\u{0e22}", // เนื้อหาภาษาไทย
        expect_refusal: None,
    },
    Entry {
        folder: "\u{0627}\u{0644}\u{0639}\u{0631}\u{0628}\u{064a}\u{0629}", // العربية
        input_name: "\u{062a}\u{0642}\u{0631}\u{064a}\u{0631}.txt",         // تقرير.txt
        content: "\u{0645}\u{062d}\u{062a}\u{0648}\u{0649} \u{0639}\u{0631}\u{0628}\u{064a}", // محتوى عربي
        expect_refusal: None,
    },
    Entry {
        folder: "\u{4e2d}\u{6587}",                                  // 中文
        input_name: "\u{62a5}\u{544a}.txt",                          // 报告.txt
        content: "\u{4e2d}\u{6587}\u{5185}\u{5bb9}\u{793a}\u{4f8b}", // 中文内容示例
        expect_refusal: None,
    },
    Entry {
        folder: "emoji",
        input_name: "\u{1f4c1}file\u{1f4c4}.txt", // 📁file📄.txt
        content: "emoji content \u{1f389}\u{1f680}", // emoji content 🎉🚀
        expect_refusal: None,
    },
    // The NFC/NFD pair (Spec §4.6, P5.3.c): stored once, as NFC, regardless
    // of which spelling was typed.
    Entry {
        folder: "nfd",
        input_name: NFD_CAFE,
        content: "stored under an NFD spelling; reported as NFC",
        expect_refusal: None,
    },
    // Windows-reserved names and characters (FR-31, P5.2.b) — refused
    // everywhere under P5.2.f's union, not only on a Windows destination.
    Entry {
        folder: "reserved",
        input_name: "CON.txt",
        content: "a reserved device name with an extension",
        expect_refusal: Some("ReservedName"),
    },
    Entry {
        folder: "reserved",
        input_name: "NUL",
        content: "a reserved device name with no extension",
        expect_refusal: Some("ReservedName"),
    },
    Entry {
        folder: "reserved",
        input_name: "COM1.log",
        content: "a reserved device name, one of the numbered set",
        expect_refusal: Some("ReservedName"),
    },
    Entry {
        folder: "reserved",
        input_name: "10:30.txt",
        content: "a reserved character",
        expect_refusal: Some("ReservedCharacter"),
    },
    Entry {
        folder: "reserved",
        input_name: "trailing.",
        content: "a trailing dot",
        expect_refusal: Some("TrailingDotOrSpace"),
    },
    Entry {
        folder: "reserved",
        input_name: "trailing ",
        content: "a trailing space",
        expect_refusal: Some("TrailingDotOrSpace"),
    },
    // A reserved *prefix* is not a reserved name (T5.6's negative case).
    Entry {
        folder: "reserved",
        input_name: "CONSOLE.txt",
        content: "not reserved; CONSOLE is not CON",
        expect_refusal: None,
    },
    // A case collision (FR-31, P5.2.e) — refused regardless of the
    // destination's own case sensitivity.
    Entry {
        folder: "case",
        input_name: "Photo.jpg",
        content: "upper-case photo bytes",
        expect_refusal: Some("CaseCollision"),
    },
    Entry {
        folder: "case",
        input_name: "photo.jpg",
        content: "lower-case photo bytes",
        expect_refusal: Some("CaseCollision"),
    },
];

#[derive(Serialize)]
struct ManifestEntry {
    folder: String,
    input_name: String,
    name: String,
    content: String,
    expect_refusal: Option<String>,
}

#[derive(Serialize)]
struct Manifest {
    password: String,
    entries: Vec<ManifestEntry>,
}

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/portability");
    let vault_dir = root.join("Fixture.veil");
    let _ = std::fs::remove_dir_all(&vault_dir);

    let password = Password::new(FIXTURE_PASSWORD.to_owned());
    let mut vault =
        Vault::create(&vault_dir, &password, KdfParams::for_tests(), 1_000_000).unwrap();

    let mut manifest_entries = Vec::with_capacity(ENTRIES.len());
    for entry in ENTRIES {
        let id = vault
            .add(
                entry.input_name,
                entry.folder,
                &mut entry.content.as_bytes(),
                &mut NoProgress,
                &Cancel::new(),
            )
            .unwrap();
        // The stored name, read back rather than assumed, so the manifest
        // records what P5.1 actually normalised it to.
        let stored = vault.entries().iter().find(|e| e.id == id).unwrap();
        manifest_entries.push(ManifestEntry {
            folder: stored.folder.clone(),
            input_name: entry.input_name.to_owned(),
            name: stored.name.clone(),
            content: entry.content.to_owned(),
            expect_refusal: entry.expect_refusal.map(str::to_owned),
        });
    }

    let manifest = Manifest {
        password: FIXTURE_PASSWORD.to_owned(),
        entries: manifest_entries,
    };
    std::fs::write(
        root.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();

    println!("Wrote {} entries to {}", ENTRIES.len(), vault_dir.display());
    println!("Manifest at {}", root.join("manifest.json").display());
}
