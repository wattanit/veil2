//! Tauri commands over `veil-core` (Spec §5.3, P5.1). Each mirrors what the
//! CLI already does for the same operation (A-4) and runs the vault work on
//! a worker thread, never on the thread servicing the webview (A-3).

use tauri::{AppHandle, Emitter, Manager, Runtime};
use veil_core::crypto::Password;
use veil_core::index::Entry;
use veil_core::{EntryId, Progress, ProgressReport, Vault};

use crate::state::AppState;

/// What `open_vault` and `open_fixture_vault` hand back once a vault is
/// open — enough for the frontend's status line, nothing it would need to
/// browse the list for.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultSummary {
    /// FR-7: the figure the frontend's status line shows immediately.
    pub entry_count: u64,
}

/// One entry, in the shape the frontend's list renders (P5.4).
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryInfo {
    /// Opaque outside this vault; the frontend sends it back unmodified to
    /// name what to extract or delete.
    pub id: u64,
    /// NFC, UTF-8 (§4.6) — already normalised by `veil-core`, not by this
    /// layer.
    pub name: String,
    /// Recorded path relative to the added root; metadata, not structure
    /// (FR-8).
    pub folder: String,
    /// Plaintext length in bytes.
    pub size: u64,
    /// When it was added, as a Unix timestamp in seconds.
    pub added_at: u64,
}

impl From<&Entry> for EntryInfo {
    fn from(entry: &Entry) -> Self {
        Self {
            id: entry.id.get(),
            name: entry.name.clone(),
            folder: entry.folder.clone(),
            size: entry.size,
            added_at: entry.added_at,
        }
    }
}

/// What a drop or an add dialog resolves to: each path either became an
/// entry or didn't, and both lists are reported rather than the whole batch
/// failing on the first bad path.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddResult {
    /// Every path that is now an entry.
    pub added: Vec<EntryInfo>,
    /// `"{path}: {reason}"` for every path that is not.
    pub failed: Vec<String>,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgressPayload {
    done: u64,
    total: Option<u64>,
}

/// Reports progress to the UI thread over Tauri's event channel rather than
/// through a command's return value, so it is visible before the operation
/// finishes (P5.1.d, A-3).
///
/// Generic over the runtime, like every command below that touches it, so
/// T5.3 can drive this against [`tauri::test::MockRuntime`] instead of a
/// real webview.
struct EventProgress<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> Progress for EventProgress<R> {
    fn report(&mut self, report: ProgressReport) {
        let payload = ProgressPayload {
            done: report.done,
            total: report.total,
        };
        let _ = self.app.emit("operation-progress", payload);
    }
}

/// Runs `f` on a worker thread and flattens the join result into the same
/// error type every command already returns, so each command site does not
/// repeat that plumbing.
async fn run_blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

fn summarize(vault: &Vault) -> VaultSummary {
    VaultSummary {
        entry_count: vault.entries().len() as u64,
    }
}

/// Opens a real vault by path and password — the GUI's equivalent of the
/// CLI's `open` (A-4).
#[tauri::command]
pub async fn open_vault<R: Runtime>(
    app: AppHandle<R>,
    path: String,
    password: String,
) -> Result<VaultSummary, String> {
    run_blocking(move || {
        let vault = Vault::open(std::path::Path::new(&path), &Password::new(password))
            .map_err(|e| e.to_string())?;
        let summary = summarize(&vault);
        app.state::<AppState>().set_vault(vault)?;
        Ok(summary)
    })
    .await
}

/// Debug-only: opens the fixture vault of `fixture::open` instead of a real
/// vault chosen by the user, so this phase has something to render before
/// Phase 6 builds the unlock screen that would otherwise be the only way in.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn open_fixture_vault<R: Runtime>(app: AppHandle<R>) -> Result<VaultSummary, String> {
    run_blocking(move || {
        let vault = crate::fixture::open().map_err(|e| e.to_string())?;
        let summary = summarize(&vault);
        app.state::<AppState>().set_vault(vault)?;
        Ok(summary)
    })
    .await
}

/// Every entry the open vault holds — the GUI's equivalent of `veil list`.
#[tauri::command]
pub async fn list_entries<R: Runtime>(app: AppHandle<R>) -> Result<Vec<EntryInfo>, String> {
    run_blocking(move || {
        app.state::<AppState>()
            .with_vault(|vault| Ok(vault.entries().iter().map(EntryInfo::from).collect()))
    })
    .await
}

/// Drops the open vault from state. Nothing is written; there is nothing to
/// flush that `add`/`replace`/`delete` did not already commit.
#[tauri::command]
pub async fn close_vault<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    run_blocking(move || app.state::<AppState>().clear_vault()).await
}

/// Reaches the same [`Cancel`](veil_core::Cancel) token the running
/// operation holds, kept in its own lock so this never waits behind the
/// operation it is meant to interrupt (P5.1.e).
#[tauri::command]
pub fn cancel_operation<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    app.state::<AppState>().cancel_current()
}

/// Writes one entry to `destination` — the GUI's equivalent of `veil
/// extract`, driven from a double-click rather than a path argument.
#[tauri::command]
pub async fn extract_entry<R: Runtime>(
    app: AppHandle<R>,
    id: u64,
    destination: String,
) -> Result<(), String> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let cancel = state.begin_cancellable()?;
        let mut progress = EventProgress { app: app.clone() };
        let result = state.with_vault(|vault| {
            vault
                .extract_to_path(
                    EntryId::new(id),
                    std::path::Path::new(&destination),
                    &mut progress,
                    &cancel,
                )
                .map_err(|e| e.to_string())
        });
        state.end_cancellable()?;
        result
    })
    .await
}

/// Adds every dropped path as a root-level entry (Design §3.3, FR-9). A
/// path that cannot be read is reported in `failed` rather than aborting the
/// whole drop — one bad path should not cost the other 33 files their add.
#[tauri::command]
pub async fn add_files<R: Runtime>(
    app: AppHandle<R>,
    paths: Vec<String>,
) -> Result<AddResult, String> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        state.with_vault_mut(|vault| {
            let mut added = Vec::new();
            let mut failed = Vec::new();
            for path in paths {
                match add_one(vault, &path) {
                    Ok(entry) => added.push(entry),
                    Err(e) => failed.push(format!("{path}: {e}")),
                }
            }
            Ok(AddResult { added, failed })
        })
    })
    .await
}

fn add_one(vault: &mut Vault, path: &str) -> Result<EntryInfo, String> {
    let source_path = std::path::Path::new(path);
    let name = source_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "path has no file name".to_owned())?;
    let mut file = std::fs::File::open(source_path).map_err(|e| e.to_string())?;
    let id = vault
        .add(
            name,
            "",
            &mut file,
            &mut veil_core::NoProgress,
            &veil_core::Cancel::new(),
        )
        .map_err(|e| e.to_string())?;
    vault
        .entries()
        .iter()
        .find(|e| e.id == id)
        .map(EntryInfo::from)
        .ok_or_else(|| "added entry not found in its own vault".to_owned())
}

/// A native save dialog for an extraction destination (Design §3.3, FR-17).
#[tauri::command]
pub async fn choose_save_path(
    app: AppHandle,
    suggested_name: String,
) -> Result<Option<String>, String> {
    run_blocking(move || {
        use tauri_plugin_dialog::DialogExt;
        let chosen = app
            .dialog()
            .file()
            .set_file_name(&suggested_name)
            .blocking_save_file();
        Ok(chosen.map(|path| path.to_string()))
    })
    .await
}
