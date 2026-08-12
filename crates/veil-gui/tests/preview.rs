//! Phase 7 test cases T7.8–T7.14 — the preview command (FR-30, C-5),
//! driven directly through Tauri's mock runtime the same way Phase 5's
//! T5.2/T5.3 and Phase 6's `conditions.rs` drive the rest of the command
//! layer.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use tauri::test::{mock_builder, mock_context, noop_assets};
use veil_core::crypto::{KdfParams, Password};
use veil_core::vault::{Cancel, NoProgress, Vault};
use veil_gui_lib::preview::{self, MAX_PREVIEW_BYTES, PreviewPayload};
use veil_gui_lib::state::AppState;

const PASSWORD: &str = "a sufficiently long password";

fn password() -> Password {
    Password::new(PASSWORD.to_owned())
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("veil2-gui-preview-{label}-{}", std::process::id()));
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

fn mock_app() -> tauri::App<tauri::test::MockRuntime> {
    mock_builder()
        .manage(AppState::default())
        .build(mock_context(noop_assets()))
        .unwrap()
}

/// Flips every byte of one entry's stored file — total damage to it, and to
/// nothing else, under one-file-per-entry storage (the CLI harness's
/// `ruin()`, restated here since this crate's tests keep no shared module).
fn ruin(path: &Path) {
    let mut bytes = std::fs::read(path).unwrap();
    for byte in &mut bytes {
        *byte ^= 0xFF;
    }
    std::fs::write(path, bytes).unwrap();
}

/// Every file under `dir`, recursively, by path relative to `dir` and its
/// exact bytes — P7.5.a's direct check that `preview_entry` writes nothing.
/// Stronger than comparing names or sizes: a file rewritten with the same
/// length would pass those and fail this.
fn snapshot_all(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let relative = path.strip_prefix(dir).unwrap().to_path_buf();
                out.insert(relative, std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

/// T7.8 — a supported, in-cap entry previews correctly, image and text
/// both.
#[test]
fn t7_8_a_supported_in_cap_entry_previews_correctly() {
    tauri::async_runtime::block_on(async {
        let scratch = Scratch::new("ok");
        let dir = scratch.vault_dir();
        let image_bytes = pattern(200);
        {
            let mut vault = Vault::create(&dir, &password(), KdfParams::for_tests()).unwrap();
            vault
                .add(
                    "photo.png",
                    "",
                    &mut image_bytes.as_slice(),
                    &mut NoProgress,
                    &Cancel::new(),
                )
                .unwrap();
            vault
                .add(
                    "notes.txt",
                    "",
                    &mut "hello, this is a note".as_bytes(),
                    &mut NoProgress,
                    &Cancel::new(),
                )
                .unwrap();
        }

        let app = mock_app();
        let handle = app.handle().clone();
        veil_gui_lib::commands::open_vault(
            handle.clone(),
            dir.to_string_lossy().into_owned(),
            PASSWORD.to_owned(),
        )
        .await
        .unwrap();

        let entries = veil_gui_lib::commands::list_entries(handle.clone())
            .await
            .unwrap();
        let photo_id = entries.iter().find(|e| e.name == "photo.png").unwrap().id;
        let notes_id = entries.iter().find(|e| e.name == "notes.txt").unwrap().id;

        let photo = preview::preview_entry(handle.clone(), photo_id)
            .await
            .unwrap();
        match photo {
            PreviewPayload::Image { mime, base64 } => {
                assert_eq!(mime, "image/png");
                assert_eq!(decode_base64(&base64), image_bytes);
            }
            PreviewPayload::Text { .. } => panic!("photo.png previewed as text"),
        }

        let notes = preview::preview_entry(handle, notes_id).await.unwrap();
        match notes {
            PreviewPayload::Text { content } => assert_eq!(content, "hello, this is a note"),
            PreviewPayload::Image { .. } => panic!("notes.txt previewed as an image"),
        }
    });
}

/// T7.9 — an unsupported extension is refused without reading ciphertext:
/// the entry's stored file is ruined first, so a refusal that still
/// succeeded, or that failed with `Corrupt` instead of `PreviewUnsupported`,
/// would prove the read happened anyway.
#[test]
fn t7_9_unsupported_extension_is_refused_without_reading_ciphertext() {
    tauri::async_runtime::block_on(async {
        let scratch = Scratch::new("unsupported");
        let dir = scratch.vault_dir();
        let entry_id = {
            let mut vault = Vault::create(&dir, &password(), KdfParams::for_tests()).unwrap();
            vault
                .add(
                    "program.exe",
                    "",
                    &mut pattern(64).as_slice(),
                    &mut NoProgress,
                    &Cancel::new(),
                )
                .unwrap()
                .get()
        };
        ruin(&veil_core::store::entry_path(
            &dir,
            veil_core::EntryId::new(entry_id),
        ));

        let app = mock_app();
        let handle = app.handle().clone();
        veil_gui_lib::commands::open_vault(
            handle.clone(),
            dir.to_string_lossy().into_owned(),
            PASSWORD.to_owned(),
        )
        .await
        .unwrap();

        let result = preview::preview_entry(handle, entry_id).await;
        assert_eq!(
            result.err().map(|e| e.kind),
            Some("PreviewUnsupported"),
            "an unsupported extension must be refused before its ruined content is ever read"
        );
    });
}

/// T7.10 — an entry above C-5's cap is refused without reading ciphertext,
/// proven the same way T7.9 proves it: the file is ruined, and a `Corrupt`
/// result here would mean the cap check did not come first.
#[test]
fn t7_10_an_entry_above_the_cap_is_refused_without_reading_ciphertext() {
    tauri::async_runtime::block_on(async {
        let scratch = Scratch::new("too-large");
        let dir = scratch.vault_dir();
        let big = pattern((MAX_PREVIEW_BYTES + 1) as usize);
        let entry_id = {
            let mut vault = Vault::create(&dir, &password(), KdfParams::for_tests()).unwrap();
            vault
                .add(
                    "big.txt",
                    "",
                    &mut big.as_slice(),
                    &mut NoProgress,
                    &Cancel::new(),
                )
                .unwrap()
                .get()
        };
        ruin(&veil_core::store::entry_path(
            &dir,
            veil_core::EntryId::new(entry_id),
        ));

        let app = mock_app();
        let handle = app.handle().clone();
        veil_gui_lib::commands::open_vault(
            handle.clone(),
            dir.to_string_lossy().into_owned(),
            PASSWORD.to_owned(),
        )
        .await
        .unwrap();

        let result = preview::preview_entry(handle, entry_id).await;
        assert_eq!(
            result.err().map(|e| e.kind),
            Some("PreviewTooLarge"),
            "an entry over C-5's cap must be refused before its ruined content is ever read"
        );
    });
}

/// T7.11 — invalid UTF-8 in a text-listed extension is refused, not
/// garbled.
#[test]
fn t7_11_invalid_utf8_in_a_text_extension_is_refused_not_garbled() {
    tauri::async_runtime::block_on(async {
        let scratch = Scratch::new("not-utf8");
        let dir = scratch.vault_dir();
        let entry_id = {
            let mut vault = Vault::create(&dir, &password(), KdfParams::for_tests()).unwrap();
            let mut invalid: &[u8] = &[0x66, 0x6f, 0xff, 0xfe, 0x6f];
            vault
                .add(
                    "notes.txt",
                    "",
                    &mut invalid,
                    &mut NoProgress,
                    &Cancel::new(),
                )
                .unwrap()
                .get()
        };

        let app = mock_app();
        let handle = app.handle().clone();
        veil_gui_lib::commands::open_vault(
            handle.clone(),
            dir.to_string_lossy().into_owned(),
            PASSWORD.to_owned(),
        )
        .await
        .unwrap();

        let result = preview::preview_entry(handle, entry_id).await;
        assert_eq!(result.err().map(|e| e.kind), Some("PreviewNotText"));
    });
}

/// T7.12 — a failed integrity check during preview is reported, not passed
/// through as content.
#[test]
fn t7_12_a_failed_integrity_check_is_reported_not_passed_through() {
    tauri::async_runtime::block_on(async {
        let scratch = Scratch::new("damaged");
        let dir = scratch.vault_dir();
        let entry_id = {
            let mut vault = Vault::create(&dir, &password(), KdfParams::for_tests()).unwrap();
            vault
                .add(
                    "notes.txt",
                    "",
                    &mut pattern(64).as_slice(),
                    &mut NoProgress,
                    &Cancel::new(),
                )
                .unwrap()
                .get()
        };
        ruin(&veil_core::store::entry_path(
            &dir,
            veil_core::EntryId::new(entry_id),
        ));

        let app = mock_app();
        let handle = app.handle().clone();
        veil_gui_lib::commands::open_vault(
            handle.clone(),
            dir.to_string_lossy().into_owned(),
            PASSWORD.to_owned(),
        )
        .await
        .unwrap();

        let result = preview::preview_entry(handle, entry_id).await;
        assert_eq!(
            result.err().map(|e| e.kind),
            Some("Corrupt"),
            "a damaged, otherwise-supported entry must fail the same way a damaged extraction does"
        );
    });
}

/// T7.13 — a successful preview touches no file in the vault directory.
#[test]
fn t7_13_a_successful_preview_touches_no_file() {
    tauri::async_runtime::block_on(async {
        let scratch = Scratch::new("snapshot-ok");
        let dir = scratch.vault_dir();
        let entry_id = {
            let mut vault = Vault::create(&dir, &password(), KdfParams::for_tests()).unwrap();
            vault
                .add(
                    "notes.txt",
                    "",
                    &mut "hello, this is a note".as_bytes(),
                    &mut NoProgress,
                    &Cancel::new(),
                )
                .unwrap()
                .get()
        };

        let app = mock_app();
        let handle = app.handle().clone();
        veil_gui_lib::commands::open_vault(
            handle.clone(),
            dir.to_string_lossy().into_owned(),
            PASSWORD.to_owned(),
        )
        .await
        .unwrap();

        let before = snapshot_all(&dir);
        preview::preview_entry(handle, entry_id).await.unwrap();
        let after = snapshot_all(&dir);
        assert_eq!(
            before, after,
            "a successful preview changed something in the vault directory"
        );
    });
}

/// T7.14 — a refused or failed preview touches no file either. Repeats
/// T7.13's snapshot around T7.9's (unsupported extension), T7.10's
/// (over the cap), and T7.12's (failed integrity check) scenarios.
#[test]
fn t7_14_a_refused_or_failed_preview_touches_no_file() {
    tauri::async_runtime::block_on(async {
        for (label, name, size, ruin_first, expected_kind) in [
            (
                "unsupported",
                "program.exe",
                64_usize,
                true,
                "PreviewUnsupported",
            ),
            (
                "too-large",
                "big.txt",
                (MAX_PREVIEW_BYTES + 1) as usize,
                true,
                "PreviewTooLarge",
            ),
            ("damaged", "notes.txt", 64, true, "Corrupt"),
        ] {
            let scratch = Scratch::new(&format!("snapshot-refused-{label}"));
            let dir = scratch.vault_dir();
            let entry_id = {
                let mut vault = Vault::create(&dir, &password(), KdfParams::for_tests()).unwrap();
                vault
                    .add(
                        name,
                        "",
                        &mut pattern(size).as_slice(),
                        &mut NoProgress,
                        &Cancel::new(),
                    )
                    .unwrap()
                    .get()
            };
            if ruin_first {
                ruin(&veil_core::store::entry_path(
                    &dir,
                    veil_core::EntryId::new(entry_id),
                ));
            }

            let app = mock_app();
            let handle = app.handle().clone();
            veil_gui_lib::commands::open_vault(
                handle.clone(),
                dir.to_string_lossy().into_owned(),
                PASSWORD.to_owned(),
            )
            .await
            .unwrap();

            let before = snapshot_all(&dir);
            let result = preview::preview_entry(handle, entry_id).await;
            let after = snapshot_all(&dir);

            assert_eq!(
                result.err().map(|e| e.kind),
                Some(expected_kind),
                "case {label:?} did not fail the way it was set up to"
            );
            assert_eq!(
                before, after,
                "case {label:?}: a refused or failed preview changed something in the vault directory"
            );
        }
    });
}

fn decode_base64(input: &str) -> Vec<u8> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let value_of = |c: u8| ALPHABET.iter().position(|&a| a == c).unwrap() as u32;

    let mut out = Vec::new();
    let bytes: Vec<u8> = input.bytes().filter(|&b| b != b'=').collect();
    for chunk in bytes.chunks(4) {
        let mut value: u32 = 0;
        for &c in chunk {
            value = (value << 6) | value_of(c);
        }
        value <<= 6 * (4 - chunk.len() as u32);
        let total_bytes = (chunk.len() * 6) / 8;
        for i in 0..total_bytes {
            out.push(((value >> (16 - 8 * i)) & 0xFF) as u8);
        }
    }
    out
}
