//! Phase 6 test cases T6.9, T6.19, T6.27, T6.28, T6.29 — the constrained
//! conditions Design §4.3 designs a response for, and the check-for-damage
//! command, driven through `tauri::test`'s mock runtime the same way
//! Phase 5's T5.2/T5.3 drive the command layer (Design §4.3, §8.6; FR-16,
//! FR-23, FR-24, FR-26; S-3).
//!
//! Two conditions in `veil_core::Error` are checked differently because
//! nothing in `veil-core` constructs them today, confirmed by reading the
//! source rather than assumed:
//! - `FormatSuperseded` needs a header version below
//!   `OLDEST_SUPPORTED_FORMAT_VERSION`, which equals `CURRENT_FORMAT_VERSION`
//!   right now — this is the first format version this codebase has ever
//!   written, so nothing is superseded by itself yet. Built here against a
//!   deliberately patched header byte, the same technique as `FormatTooNew`.
//! - `StorageUnavailable` has no construction site anywhere in `veil-core`
//!   at all (`From<std::io::Error>` maps no `ErrorKind` to it) — there is no
//!   fixture that reaches it, deliberate or otherwise. Checked as a direct
//!   mapping (`ErrorInfo::from(Error::StorageUnavailable)`) instead of
//!   through a triggered condition, so the day something does construct it,
//!   the frontend already has a `kind` ready.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tauri::test::{mock_builder, mock_context, noop_assets};
use tauri::{Listener as _, Manager as _};
use veil_core::crypto::{KdfParams, Password};
use veil_core::vault::{Cancel, Limits, NoProgress, Vault};
use veil_gui_lib::commands;
use veil_gui_lib::errors::ErrorInfo;
use veil_gui_lib::state::AppState;

const PASSWORD: &str = "a sufficiently long password";

fn password() -> Password {
    Password::new(PASSWORD.to_owned())
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("veil2-gui-cond-{label}-{}", std::process::id()));
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

/// Every file directly under `dir` whose name starts with `index.` — the
/// index slots, snapshotted so a later write can be rolled back onto them
/// (`veil-core`'s own `tests/lifecycle.rs` uses the same technique).
fn snapshot_index(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("index.") {
            out.insert(name, std::fs::read(entry.path()).unwrap());
        }
    }
    out
}

fn restore_index(dir: &Path, snapshot: &BTreeMap<String, Vec<u8>>) {
    for (name, bytes) in snapshot {
        std::fs::write(dir.join(name), bytes).unwrap();
    }
}

/// T6.9 — a header version above what this release understands, and one
/// below what it still supports, each produce the message Design §5
/// requires — built against a deliberately patched header byte, since no
/// vault this codebase writes today is superseded by itself.
#[test]
fn t6_9_format_too_new_and_superseded() {
    tauri::async_runtime::block_on(async {
        for (patched_version, expected_kind) in
            [(9999_u16, "FormatTooNew"), (0_u16, "FormatSuperseded")]
        {
            let scratch = Scratch::new(&format!("format-{patched_version}"));
            let dir = scratch.vault_dir();
            drop(Vault::create(&dir, &password(), KdfParams::for_tests()).unwrap());

            let header_path = dir.join("veil.header");
            let mut bytes = std::fs::read(&header_path).unwrap();
            bytes[8..10].copy_from_slice(&patched_version.to_le_bytes());
            std::fs::write(&header_path, bytes).unwrap();

            let app = mock_app();
            let result = commands::open_vault(
                app.handle().clone(),
                dir.to_string_lossy().into_owned(),
                PASSWORD.to_owned(),
            )
            .await;

            assert_eq!(
                result.err().map(|e| e.kind),
                Some(expected_kind),
                "patched version {patched_version} did not map to {expected_kind}"
            );
        }
    });
}

/// T6.19 — a vault already held open refuses a second open, naming
/// `VaultInUse` (Design §4.3, FR-23).
#[test]
fn t6_19_vault_in_use() {
    tauri::async_runtime::block_on(async {
        let scratch = Scratch::new("vault-in-use");
        let dir = scratch.vault_dir();
        drop(Vault::create(&dir, &password(), KdfParams::for_tests()).unwrap());

        let app = mock_app();
        let handle = app.handle().clone();
        commands::open_vault(
            handle.clone(),
            dir.to_string_lossy().into_owned(),
            PASSWORD.to_owned(),
        )
        .await
        .unwrap();

        // The first open's Vault is still held in this same AppState — the
        // lock is real, not simulated, the same way `veil-core`'s own
        // `t2_x_a_second_open_is_refused`-style case holds one handle and
        // opens a second on top of it.
        let second = commands::open_vault(
            handle,
            dir.to_string_lossy().into_owned(),
            PASSWORD.to_owned(),
        )
        .await;
        assert_eq!(second.err().map(|e| e.kind), Some("VaultInUse"));
    });
}

/// T6.19, T6.3 — a read-only vault opens with `access: "readOnly"`, and a
/// write against it is refused naming `ReadOnly` (Design §4.3, FR-23).
#[test]
#[cfg(unix)]
fn t6_19_read_only() {
    use std::os::unix::fs::PermissionsExt;

    tauri::async_runtime::block_on(async {
        let scratch = Scratch::new("read-only");
        let dir = scratch.vault_dir();
        {
            let mut vault = Vault::create(&dir, &password(), KdfParams::for_tests()).unwrap();
            vault
                .add(
                    "kept.bin",
                    "",
                    &mut pattern(200).as_slice(),
                    &mut NoProgress,
                    &Cancel::new(),
                )
                .unwrap();
        }

        let mut denied = Vec::new();
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_file() {
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
                denied.push(path);
            }
        }
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

        if std::fs::File::create(dir.join("probe")).is_ok() {
            let _ = std::fs::remove_file(dir.join("probe"));
            restore_permissions(&dir, &denied);
            eprintln!("t6_19_read_only skipped: this account can write regardless of permissions");
            return;
        }

        let app = mock_app();
        let handle = app.handle().clone();
        let summary = commands::open_vault(
            handle.clone(),
            dir.to_string_lossy().into_owned(),
            PASSWORD.to_owned(),
        )
        .await
        .unwrap();
        assert_eq!(summary.access, "readOnly");

        let write = commands::add_files(
            handle,
            vec![dir.join("kept.bin").to_string_lossy().into_owned()],
        )
        .await;
        // `add_files` collects per-path failures rather than erroring the
        // whole command (P5.6.b's design) — the failure string names the
        // condition either way.
        let result = write.unwrap();
        assert!(result.added.is_empty());
        assert!(
            result
                .failed
                .iter()
                .any(|f| f.contains("read-only") || f.contains("cannot be changed")),
            "read-only add failure did not name the condition: {:?}",
            result.failed
        );

        restore_permissions(&dir, &denied);
    });
}

#[cfg(unix)]
fn restore_permissions(dir: &Path, files: &[PathBuf]) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
    for path in files {
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644));
    }
}

/// T6.19 — an external write underneath an open handle is never written
/// over; the next write is refused naming `ChangedOnDisk` (Design §4.3,
/// FR-24). Same generation-rollback technique as `veil-core`'s own
/// `tests/lifecycle.rs`.
#[test]
fn t6_19_changed_on_disk() {
    tauri::async_runtime::block_on(async {
        let scratch = Scratch::new("changed-on-disk");
        let dir = scratch.vault_dir();

        let older = {
            let mut vault = Vault::create(&dir, &password(), KdfParams::for_tests()).unwrap();
            vault
                .add(
                    "a.bin",
                    "",
                    &mut pattern(50).as_slice(),
                    &mut NoProgress,
                    &Cancel::new(),
                )
                .unwrap();
            snapshot_index(&dir)
        };
        let newer = {
            let mut vault = Vault::open(&dir, &password()).unwrap();
            vault
                .add(
                    "b.bin",
                    "",
                    &mut pattern(50).as_slice(),
                    &mut NoProgress,
                    &Cancel::new(),
                )
                .unwrap();
            snapshot_index(&dir)
        };

        restore_index(&dir, &older);
        let app = mock_app();
        let handle = app.handle().clone();
        commands::open_vault(
            handle.clone(),
            dir.to_string_lossy().into_owned(),
            PASSWORD.to_owned(),
        )
        .await
        .unwrap();

        restore_index(&dir, &newer);

        let source = scratch.0.join("c.bin");
        std::fs::write(&source, pattern(10)).unwrap();
        let result = commands::add_files(handle, vec![source.to_string_lossy().into_owned()])
            .await
            .unwrap();
        assert!(result.added.is_empty());
        assert!(
            result.failed.iter().any(|f| f.contains("changed on disk")),
            "expected a changed-on-disk failure, got {:?}",
            result.failed
        );
    });
}

/// T6.19 — exceeding the per-vault entry limit names the limit and the
/// value that would result (Design §4.3, FR-16).
#[test]
fn t6_19_limit_exceeded() {
    tauri::async_runtime::block_on(async {
        let scratch = Scratch::new("limit-exceeded");
        let dir = scratch.vault_dir();
        let mut vault = Vault::create(&dir, &password(), KdfParams::for_tests()).unwrap();
        vault.set_limits(Limits {
            max_entries: 0,
            ..Limits::default()
        });

        let app = mock_app();
        app.state::<AppState>().set_vault(vault).unwrap();

        let source = scratch.0.join("over.bin");
        std::fs::write(&source, pattern(10)).unwrap();
        let result = commands::add_files(
            app.handle().clone(),
            vec![source.to_string_lossy().into_owned()],
        )
        .await
        .unwrap();
        assert!(result.added.is_empty());
        assert!(
            result
                .failed
                .iter()
                .any(|f| f.contains("entries per vault")),
            "expected a limit-exceeded failure naming the limit, got {:?}",
            result.failed
        );
    });
}

/// T6.19 — a damaged entry's extraction is refused naming `Corrupt`,
/// without disturbing any other entry (Design §4.3, S-3).
#[test]
fn t6_19_damaged_entry_extraction() {
    tauri::async_runtime::block_on(async {
        let scratch = Scratch::new("damaged-extraction");
        let dir = scratch.vault_dir();
        let lost_id = {
            let mut vault = Vault::create(&dir, &password(), KdfParams::for_tests()).unwrap();
            let id = vault
                .add(
                    "lost.bin",
                    "",
                    &mut pattern(200).as_slice(),
                    &mut NoProgress,
                    &Cancel::new(),
                )
                .unwrap();
            vault
                .add(
                    "safe.bin",
                    "",
                    &mut pattern(200).as_slice(),
                    &mut NoProgress,
                    &Cancel::new(),
                )
                .unwrap();
            id.get()
        };
        std::fs::remove_file(veil_core::store::entry_path(
            &dir,
            veil_core::EntryId::new(lost_id),
        ))
        .unwrap();

        let app = mock_app();
        let handle = app.handle().clone();
        commands::open_vault(
            handle.clone(),
            dir.to_string_lossy().into_owned(),
            PASSWORD.to_owned(),
        )
        .await
        .unwrap();

        let entries = commands::list_entries(handle.clone()).await.unwrap();
        let lost = entries.iter().find(|e| e.id == lost_id).unwrap();
        assert!(
            lost.unreadable,
            "the missing entry was not marked unreadable"
        );
        let safe = entries.iter().find(|e| e.name == "safe.bin").unwrap();
        assert!(!safe.unreadable, "an unrelated entry was marked unreadable");

        let destination = scratch.0.join("out.bin");
        let result =
            commands::extract_entry(handle, lost_id, destination.to_string_lossy().into_owned())
                .await;
        assert_eq!(result.err().map(|e| e.kind), Some("Corrupt"));
    });
}

/// `StorageUnavailable` has no construction site in `veil-core` today —
/// checked as a direct mapping rather than a triggered condition.
#[test]
fn t6_19_storage_unavailable_maps_to_its_own_kind() {
    let info = ErrorInfo::from(veil_core::Error::StorageUnavailable);
    assert_eq!(info.kind, "StorageUnavailable");
}

/// T6.27 — checking a vault reports progress per entry and names no
/// failures on an intact vault (Design §8.6, FR-26).
#[test]
fn t6_27_check_vault_reports_progress_on_an_intact_vault() {
    tauri::async_runtime::block_on(async {
        let scratch = Scratch::new("check-clean");
        let dir = scratch.vault_dir();
        {
            let mut vault = Vault::create(&dir, &password(), KdfParams::for_tests()).unwrap();
            for i in 0..5 {
                vault
                    .add(
                        &format!("f{i}.bin"),
                        "",
                        &mut pattern(1000).as_slice(),
                        &mut NoProgress,
                        &Cancel::new(),
                    )
                    .unwrap();
            }
        }

        let app = mock_app();
        let handle = app.handle().clone();
        commands::open_vault(
            handle.clone(),
            dir.to_string_lossy().into_owned(),
            PASSWORD.to_owned(),
        )
        .await
        .unwrap();

        let progress_events: Arc<Mutex<Vec<()>>> = Arc::new(Mutex::new(Vec::new()));
        let counted = progress_events.clone();
        app.listen_any("operation-progress", move |_event| {
            counted.lock().unwrap().push(());
        });

        let report = commands::check_vault(handle).await.unwrap();
        assert_eq!(report.checked, 5);
        assert!(report.complete);
        assert!(report.failures.is_empty());
        assert!(
            !progress_events.lock().unwrap().is_empty(),
            "no progress event reached the UI-thread event channel during a check"
        );
    });
}

/// T6.29 — a damaged entry is named in the check report, by id, name,
/// folder, and the kind of damage (Design §8.6, S-3).
#[test]
fn t6_29_check_vault_names_a_damaged_entry() {
    tauri::async_runtime::block_on(async {
        let scratch = Scratch::new("check-damaged");
        let dir = scratch.vault_dir();
        let lost_id = {
            let mut vault = Vault::create(&dir, &password(), KdfParams::for_tests()).unwrap();
            let id = vault
                .add(
                    "lost.bin",
                    "docs",
                    &mut pattern(200).as_slice(),
                    &mut NoProgress,
                    &Cancel::new(),
                )
                .unwrap();
            vault
                .add(
                    "safe.bin",
                    "",
                    &mut pattern(200).as_slice(),
                    &mut NoProgress,
                    &Cancel::new(),
                )
                .unwrap();
            id.get()
        };
        std::fs::remove_file(veil_core::store::entry_path(
            &dir,
            veil_core::EntryId::new(lost_id),
        ))
        .unwrap();

        let app = mock_app();
        let handle = app.handle().clone();
        commands::open_vault(
            handle.clone(),
            dir.to_string_lossy().into_owned(),
            PASSWORD.to_owned(),
        )
        .await
        .unwrap();

        let report = commands::check_vault(handle).await.unwrap();
        assert_eq!(report.checked, 2);
        assert_eq!(report.failures.len(), 1);
        let failure = &report.failures[0];
        assert_eq!(failure.id, lost_id);
        assert_eq!(failure.name, "lost.bin");
        assert_eq!(failure.folder, "docs");
        assert!(!failure.damage.is_empty());
    });
}

/// T6.28 — a check cancelled partway reports what it completed rather than
/// discarding the result (Design §8.6). Waits for the first progress event
/// before cancelling, the same synchronisation `veil-core`'s own crash
/// tests and this crate's T5.3 use — cancelling before the operation has
/// registered its token would silently cancel nothing.
#[test]
fn t6_28_a_cancelled_check_reports_what_it_completed() {
    tauri::async_runtime::block_on(async {
        let scratch = Scratch::new("check-cancel");
        let dir = scratch.vault_dir();
        {
            let mut vault = Vault::create(&dir, &password(), KdfParams::for_tests()).unwrap();
            for i in 0..200 {
                vault
                    .add(
                        &format!("f{i}.bin"),
                        "",
                        &mut pattern(64 * 1024).as_slice(),
                        &mut NoProgress,
                        &Cancel::new(),
                    )
                    .unwrap();
            }
        }

        let app = mock_app();
        let handle = app.handle().clone();
        commands::open_vault(
            handle.clone(),
            dir.to_string_lossy().into_owned(),
            PASSWORD.to_owned(),
        )
        .await
        .unwrap();

        let progress_events: Arc<Mutex<Vec<()>>> = Arc::new(Mutex::new(Vec::new()));
        let counted = progress_events.clone();
        app.listen_any("operation-progress", move |_event| {
            counted.lock().unwrap().push(());
        });

        let check_handle = handle.clone();
        let check =
            tauri::async_runtime::spawn(async move { commands::check_vault(check_handle).await });

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while progress_events.lock().unwrap().is_empty() {
            assert!(
                std::time::Instant::now() < deadline,
                "no progress event arrived to synchronise on"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        commands::cancel_operation(handle).unwrap();

        let report = check.await.unwrap().unwrap();
        assert!(
            !report.complete,
            "a check cancelled in flight reported complete"
        );
        assert!(
            report.checked < 200,
            "a check cancelled in flight still examined every entry"
        );
    });
}
