//! A debug-only fixture vault for P5.5's complex-script rendering check.
//!
//! Never compiled into a release build — this whole module is behind
//! `cfg(debug_assertions)`, mirroring how `KdfParams::for_tests()` is
//! compiled out of `veil-core`'s release builds (P1.1.d). It exists so
//! Phase 5 has something to render before Phase 6 builds the real unlock
//! and vault-creation flows.

use std::io::Cursor;

use veil_core::crypto::{KdfParams, Password};
use veil_core::vault::Vault;

const FIXTURE_PASSWORD: &str = "phase-5-fixture-password-not-for-real-use";

/// (name, folder). Thai, Arabic, Han, and an emoji — the four scripts
/// Design §2.2 and Spec §5.3 name as the evidence for choosing Tauri.
const FIXTURE_ENTRIES: &[(&str, &str)] = &[
    ("รายงานประจำปี.pdf", ""),
    ("تقرير سنوي.pdf", ""),
    ("年次報告書.pdf", "2026"),
    ("summary 📎.pdf", "2026"),
];

/// Opens the fixture vault, creating and populating it first if this is the
/// first call.
pub fn open() -> veil_core::Result<Vault> {
    let dir = std::env::temp_dir().join("veil2-gui-phase5-fixture");
    if let Ok(vault) = Vault::open(&dir, &password()) {
        return Ok(vault);
    }

    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    let mut vault = Vault::create(&dir, &password(), KdfParams::for_tests())?;
    for (name, folder) in FIXTURE_ENTRIES {
        let mut content = Cursor::new(b"fixture content, not real data".to_vec());
        vault.add(
            name,
            folder,
            &mut content,
            &mut veil_core::NoProgress,
            &veil_core::Cancel::new(),
        )?;
    }
    drop(vault);
    Vault::open(&dir, &password())
}

fn password() -> Password {
    Password::new(FIXTURE_PASSWORD.to_owned())
}
