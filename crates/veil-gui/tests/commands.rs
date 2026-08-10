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

use tauri::test::{mock_builder, mock_context, noop_assets};
use tauri::{Listener as _, Manager as _};
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
        assert_eq!(
            result.unwrap_err().kind,
            "Cancelled",
            "the error from a cancelled extract was not kind Cancelled"
        );
        assert!(
            !progress_events.lock().unwrap().is_empty(),
            "no progress event reached the UI-thread event channel before cancellation"
        );
    });
}

/// T6.20 — a dropped folder is walked (FR-10), not handed to the single-file
/// add path as if it were a file's content.
///
/// Regression test: confirmed live that dropping a folder silently added
/// nothing — `add_files` tried to `File::open` the directory and read it,
/// which fails, and the failure never reached anything the person dropping
/// it could see. `add_folder` is a different `veil-core` method entirely,
/// and nothing called it until this fix.
#[test]
fn t6_20_a_dropped_folder_is_walked_not_read_as_a_file() {
    tauri::async_runtime::block_on(async {
        let scratch = Scratch::new("add-folder");
        let dir = scratch.vault_dir();
        let vault = Vault::create(&dir, &password(), KdfParams::for_tests()).unwrap();

        let source_root = scratch.0.join("Photos");
        std::fs::create_dir_all(source_root.join("2024")).unwrap();
        std::fs::write(source_root.join("2024").join("a.jpg"), pattern(100)).unwrap();
        std::fs::write(source_root.join("b.jpg"), pattern(50)).unwrap();

        let app = mock_app();
        app.state::<AppState>().set_vault(vault).unwrap();

        let result = commands::add_files(
            app.handle().clone(),
            vec![source_root.to_string_lossy().into_owned()],
        )
        .await
        .unwrap();

        assert_eq!(
            result.added.len(),
            2,
            "expected both files under the dropped folder to be added, got {:?}",
            result.added
        );
        assert!(
            result.failed.is_empty(),
            "unexpected failures: {:?}",
            result.failed
        );
        assert!(
            result
                .added
                .iter()
                .any(|e| e.name == "a.jpg" && e.folder == "Photos/2024")
        );
        assert!(
            result
                .added
                .iter()
                .any(|e| e.name == "b.jpg" && e.folder == "Photos")
        );
    });
}

/// A colliding path is reported for confirmation, not replaced outright and
/// not failed outright — and the match is on folder *and* name together, so
/// `FolderA/x.bin` never collides with `FolderB/x.bin` (Design §8.7, FR-14).
///
/// Regression test: the interaction this replaced (dropping a new file onto
/// an existing row) proved unreliable live — Tauri's own drag-position data
/// was wrong regardless of where in the window the cursor actually was, and
/// the standard DOM drag events never fired at all (the webview's own
/// handler consumes the OS drag first) — so there is no reliable position
/// to detect a row from at all. Matching by the dropped file's own identity
/// needs no position data.
#[test]
fn t6_33_a_colliding_path_is_held_for_confirmation_matched_by_folder_and_name() {
    tauri::async_runtime::block_on(async {
        let scratch = Scratch::new("add-collision");
        let dir = scratch.vault_dir();
        let vault = Vault::create(&dir, &password(), KdfParams::for_tests()).unwrap();

        let app = mock_app();
        app.state::<AppState>().set_vault(vault).unwrap();
        let handle = app.handle().clone();

        // Dropping "root1" (containing FolderA/x.bin) records the entry's
        // folder as "root1/FolderA" — the dropped root's own name is the
        // top-level segment (FR-10), so identity is stable regardless of
        // where on disk the root happened to sit, and two *different* roots
        // named "FolderA" do not collide just because both happen to sit
        // directly under some drop point.
        let root1 = scratch.0.join("root1");
        std::fs::create_dir_all(root1.join("FolderA")).unwrap();
        std::fs::write(root1.join("FolderA").join("x.bin"), pattern(10)).unwrap();
        let seed = commands::add_files(handle.clone(), vec![root1.to_string_lossy().into_owned()])
            .await
            .unwrap();
        assert_eq!(seed.added.len(), 1);
        assert!(seed.collisions.is_empty());
        assert_eq!(seed.added[0].folder, "root1/FolderA");

        // "root2" containing FolderB/x.bin does not collide — different
        // folder, same name — and is added normally.
        let root2 = scratch.0.join("root2");
        std::fs::create_dir_all(root2.join("FolderB")).unwrap();
        std::fs::write(root2.join("FolderB").join("x.bin"), pattern(20)).unwrap();
        let distinct =
            commands::add_files(handle.clone(), vec![root2.to_string_lossy().into_owned()])
                .await
                .unwrap();
        assert_eq!(
            distinct.added.len(),
            1,
            "root2/FolderB/x.bin wrongly collided with root1/FolderA/x.bin: {:?}",
            distinct.collisions
        );
        assert!(distinct.collisions.is_empty());

        // A *different* root elsewhere on disk, but with the same basename
        // "root1" and the same FolderA/x.bin beneath it, does collide — the
        // identity is the stored folder-and-name pair, not the source path.
        let root3 = scratch.0.join("nested").join("root1");
        std::fs::create_dir_all(root3.join("FolderA")).unwrap();
        std::fs::write(root3.join("FolderA").join("x.bin"), pattern(30)).unwrap();
        let collided = commands::add_files(handle, vec![root3.to_string_lossy().into_owned()])
            .await
            .unwrap();
        assert!(collided.added.is_empty());
        assert_eq!(collided.collisions.len(), 1);
        assert_eq!(collided.collisions[0].folder, "root1/FolderA");
        assert_eq!(collided.collisions[0].name, "x.bin");
    });
}

/// T6.37 — a collision partway through a dropped folder does not cost the
/// rest of it: `Vault::add_folder` returns on the first error, which is
/// why `add_files` walks the folder itself instead of calling it.
#[test]
fn t6_37_a_collision_partway_through_a_folder_does_not_abort_the_rest() {
    tauri::async_runtime::block_on(async {
        let scratch = Scratch::new("add-folder-collision");
        let dir = scratch.vault_dir();
        let vault = Vault::create(&dir, &password(), KdfParams::for_tests()).unwrap();

        let app = mock_app();
        app.state::<AppState>().set_vault(vault).unwrap();
        let handle = app.handle().clone();

        // Seed one existing entry via a root named "shared": "shared/e.bin".
        let seed_root = scratch.0.join("first").join("shared");
        std::fs::create_dir_all(&seed_root).unwrap();
        std::fs::write(seed_root.join("e.bin"), pattern(5)).unwrap();
        commands::add_files(
            handle.clone(),
            vec![seed_root.to_string_lossy().into_owned()],
        )
        .await
        .unwrap();

        // A different root, elsewhere on disk but with the same basename
        // "shared", holding ten files — one of which ("e.bin") collides
        // with the seed above once its own root name is prepended.
        let batch = scratch.0.join("second").join("shared");
        std::fs::create_dir_all(&batch).unwrap();
        for i in 0..10 {
            std::fs::write(batch.join(format!("f{i}.bin")), pattern(10)).unwrap();
        }
        std::fs::write(batch.join("e.bin"), pattern(99)).unwrap();

        let result = commands::add_files(handle, vec![batch.to_string_lossy().into_owned()])
            .await
            .unwrap();

        assert_eq!(
            result.added.len(),
            10,
            "the nine non-colliding files plus nothing else should have been added, got {:?}",
            result.added
        );
        assert_eq!(result.collisions.len(), 1);
        assert_eq!(result.collisions[0].name, "e.bin");
        assert!(result.failed.is_empty());
    });
}
