//! Phase 5 test cases T5.2 and T5.3 — the command layer, driven directly
//! through Tauri's mock runtime rather than a real webview (Spec §5.3, A-3,
//! A-4).
//!
//! Layout, colour, script rendering, and drag-and-drop are properties of a
//! real webview and are checked by hand instead (Phase5-TestCases.md); what
//! is ordinary Rust — thread placement, progress events, argument shape — is
//! checked the same way `veil-core`'s own API is, by calling it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::Listener as _;
use tauri::test::{mock_builder, mock_context, noop_assets};
use veil_core::crypto::{KdfParams, Password};
use veil_core::vault::{Cancel, NoProgress, Vault};
use veil_gui_lib::state::AppState;
use veil_gui_lib::{commands, fixture};

const PASSWORD: &str = "a sufficiently long password";

fn password() -> Password {
    Password::new(PASSWORD.to_owned())
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("veil2-gui-test-{label}-{}", std::process::id()));
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

/// T5.2 — every GUI command mirrors its CLI equivalent (P5.1.b, A-4).
///
/// Uses the same fixture the shell opens in debug builds (`fixture::open`),
/// so this also stands as a compile-time check that the fixture module
/// (P5.5.a) still builds an openable vault.
#[test]
fn t5_2_every_command_mirrors_the_library_it_wraps() {
    tauri::async_runtime::block_on(async {
        let app = mock_app();
        let handle = app.handle().clone();

        let direct = fixture::open().unwrap();
        let expected_count = direct.entries().len() as u64;
        drop(direct);

        let summary = commands::open_fixture_vault(handle.clone()).await.unwrap();
        assert_eq!(summary.entry_count, expected_count);

        let entries = commands::list_entries(handle.clone()).await.unwrap();
        assert_eq!(entries.len() as u64, expected_count);
        assert!(
            entries.iter().any(|e| e.name.contains("รายงาน")),
            "the Thai fixture name did not survive the command round trip: {entries:?}"
        );

        commands::close_vault(handle.clone()).await.unwrap();
        let after_close = commands::list_entries(handle).await;
        assert!(
            after_close.is_err(),
            "list_entries succeeded after close_vault with nothing open"
        );
    });
}

/// T5.3 — a running operation reports progress and can be cancelled without
/// waiting behind itself (P5.1.c, P5.1.d, P5.1.e, A-3, FR-15, FR-20).
#[test]
fn t5_3_progress_arrives_and_cancellation_is_not_blocked_by_the_operation() {
    tauri::async_runtime::block_on(async {
        let scratch = Scratch::new("progress-cancel");
        let dir = scratch.vault_dir();
        let id = {
            let mut vault = Vault::create(&dir, &password(), KdfParams::for_tests()).unwrap();
            let id = vault
                .add(
                    "large.bin",
                    "",
                    &mut pattern(16 * 1024 * 1024).as_slice(),
                    &mut NoProgress,
                    &Cancel::new(),
                )
                .unwrap();
            id.get()
        };

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

        let destination = scratch.0.join("out.bin");
        let extract_handle = handle.clone();
        let destination_str = destination.to_string_lossy().into_owned();
        let extraction = tauri::async_runtime::spawn(async move {
            commands::extract_entry(extract_handle, id, destination_str).await
        });

        // Wait for the operation to have genuinely registered its token and
        // reported at least one chunk — cancelling before that races against
        // `begin_cancellable` and would silently cancel nothing (the same
        // reason `veil-core`'s own crash tests wait for bytes to appear on
        // disk before killing the process, rather than killing immediately).
        let wait_deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while progress_events.lock().unwrap().is_empty() {
            assert!(
                std::time::Instant::now() < wait_deadline,
                "no progress event arrived within 5s to synchronise the cancel on"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        // Cancel promptly rather than waiting for completion — the point of
        // P5.1.e is that this call does not queue behind the extract's own
        // lock.
        let cancel_start = std::time::Instant::now();
        commands::cancel_operation(handle).unwrap();
        let cancel_elapsed = cancel_start.elapsed();

        let result = extraction.await.unwrap();

        assert!(
            cancel_elapsed < std::time::Duration::from_millis(500),
            "cancel_operation took {cancel_elapsed:?} — it should never wait on the vault lock"
        );
        assert!(
            result.is_err(),
            "a 16 MiB extract cancelled immediately after starting still ran to completion"
        );
        assert!(
            result.unwrap_err().contains("cancel"),
            "the error from a cancelled extract did not name cancellation"
        );
        assert!(
            !progress_events.lock().unwrap().is_empty(),
            "no progress event reached the UI-thread event channel before cancellation"
        );
    });
}
