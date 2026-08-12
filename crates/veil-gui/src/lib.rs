//! Tauri v2 shell over `veil-core` (Spec §5.3). Phase 5 built the shell and
//! the security configuration; Phase 6 builds the product around it — the
//! unlock screen, vault creation, and every other requirement reachable
//! from the GUI (A-4).

// `pub` so `tests/` can drive the command layer directly through
// `tauri::test`'s mock runtime (T5.2, T5.3) — the same "call it through the
// public surface" discipline `veil-core`'s own integration tests follow.
pub mod commands;
pub mod errors;
#[cfg(debug_assertions)]
pub mod fixture;
pub mod preview;
pub mod state;

/// Creates the main window with non-persistent storage (P5.2.a, Spec §5.3,
/// HC-1) — on macOS, `incognito(true)` is what selects the `WKWebView`'s
/// `nonPersistentDataStore` rather than the default one. `tauri.conf.json`'s
/// declarative `app.windows` has no field for this, which is why the window
/// is built here instead of listed there.
fn create_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("index.html".into()))
        .title("Veil2")
        .inner_size(900.0, 600.0)
        .resizable(true)
        .incognito(true)
        // A native overlay title bar (traffic lights floating over the
        // content, no title text) instead of the plain default bar — the
        // frontend reserves a matching drag strip (`#title-bar`) so the
        // window stays movable without a visible title area.
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .build()?;
    Ok(())
}

/// Builds and runs the application. Exits the process if the runtime fails
/// to start; there is no recovery available at this level.
pub fn run() {
    #[cfg(debug_assertions)]
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::default())
        .setup(|app| create_main_window(app.handle()).map_err(Into::into))
        .invoke_handler(tauri::generate_handler![
            commands::open_vault,
            commands::open_fixture_vault,
            commands::create_vault,
            commands::choose_vault_path,
            commands::list_entries,
            commands::extract_entry,
            commands::close_vault,
            commands::cancel_operation,
            commands::add_files,
            commands::choose_save_path,
            commands::choose_source_paths,
            commands::delete_entry,
            commands::replace_entry,
            commands::change_password,
            commands::check_vault,
            preview::preview_entry,
        ]);

    #[cfg(not(debug_assertions))]
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::default())
        .setup(|app| create_main_window(app.handle()).map_err(Into::into))
        .invoke_handler(tauri::generate_handler![
            commands::open_vault,
            commands::create_vault,
            commands::choose_vault_path,
            commands::list_entries,
            commands::extract_entry,
            commands::close_vault,
            commands::cancel_operation,
            commands::add_files,
            commands::choose_save_path,
            commands::choose_source_paths,
            commands::delete_entry,
            commands::replace_entry,
            commands::change_password,
            commands::check_vault,
            preview::preview_entry,
        ]);

    if let Err(error) = builder.run(tauri::generate_context!()) {
        eprintln!("veil-gui: {error}");
        std::process::exit(1);
    }
}
