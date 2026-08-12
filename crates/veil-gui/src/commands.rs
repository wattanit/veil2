//! Tauri commands over `veil-core` (Spec §5.3, P5.1, P6.0). Each mirrors
//! what the CLI already does for the same operation (A-4) and runs the
//! vault work on a worker thread, never on the thread servicing the webview
//! (A-3). Every error is a structured [`ErrorInfo`] rather than a bare
//! `String` (P6.0.a, Design §4.2, §4.3) — the frontend needs to know *which*
//! condition occurred, not only read English built for a person.

use tauri::{AppHandle, Emitter, Manager, Runtime};
use veil_core::crypto::{KdfParams, Password};
use veil_core::index::{Entry, EntryId};
use veil_core::vault::{Access, Outcome};
use veil_core::{Progress, ProgressReport, Vault};

use crate::errors::{ErrorInfo, internal};
use crate::state::AppState;

/// What `open_vault`, `open_fixture_vault`, and `create_vault` hand back
/// once a vault is open — enough for the unlock screen and identity bar
/// (P6.0.d), nothing the list needs to be browsed for.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultSummary {
    /// FR-7: the figure the frontend's status line shows immediately.
    pub entry_count: u64,
    /// `"readWrite"` or `"readOnly"` — shown in the identity bar the
    /// moment the vault opens (Design §4.3), not discovered when a write
    /// fails.
    pub access: &'static str,
    /// S-3: how many entries are already known unreadable. Zero for an
    /// intact vault; the list itself names which ones via `EntryInfo`.
    pub unreadable_count: u64,
}

/// One entry, in the shape the frontend's list renders (P5.4).
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryInfo {
    /// Opaque outside this vault; the frontend sends it back unmodified to
    /// name what to extract, replace, or delete.
    pub id: u64,
    /// NFC, UTF-8 (§4.6) — already normalised by `veil-core`, not by this
    /// layer.
    pub name: String,
    /// Recorded path relative to the added root; metadata, not structure
    /// (FR-8).
    pub folder: String,
    /// Plaintext length in bytes.
    pub size: u64,
    /// When it was added (or last replaced, P6.16.d), as a Unix timestamp
    /// in seconds.
    pub added_at: u64,
    /// Whether this entry's own file is known missing or unreadable
    /// (S-3) — set from `Vault::unreadable_entries`, not from reading the
    /// content.
    pub unreadable: bool,
}

impl EntryInfo {
    fn from_entry(entry: &Entry, unreadable: bool) -> Self {
        Self {
            id: entry.id.get(),
            name: entry.name.clone(),
            folder: entry.folder.clone(),
            size: entry.size,
            added_at: entry.added_at,
            unreadable,
        }
    }
}

/// A dropped or chosen path whose name and folder already match an entry
/// this vault holds — a replace candidate, held for confirmation rather
/// than acted on immediately (Design §8.7, §4.1's irreversible-action rule)
/// or silently refused the way a bare `add` would refuse it (FR-14).
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collision {
    /// Where to read the new content from if this is confirmed.
    pub path: String,
    /// The colliding entry's name — identity, not the dropped file's own
    /// name, though the two match by definition of colliding.
    pub name: String,
    /// The colliding entry's folder, alongside `name` in the match.
    pub folder: String,
}

/// What a drop or an add dialog resolves to: each path became an entry,
/// collided with one already there, or failed outright — three lists
/// rather than the whole batch failing on the first bad path.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddResult {
    /// Every path that is now an entry.
    pub added: Vec<EntryInfo>,
    /// Every path matching an existing entry's folder and name exactly —
    /// matched on both together, never name alone, so `FolderA/x` never
    /// collides with `FolderB/x`.
    pub collisions: Vec<Collision>,
    /// `"{path}: {reason}"` for every path that is neither of the above.
    pub failed: Vec<String>,
}

/// One damaged entry, named for the check-report list (Design §8.6, S-3).
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckFailure {
    /// Identifies the entry; not used to read it — S-3 requires naming a
    /// failure without reading content.
    pub id: u64,
    /// The name the failure list shows (Design §8.6's example: `IMG_4417.raw`).
    pub name: String,
    /// The folder alongside the name, disambiguating same-named files.
    pub folder: String,
    /// `Damaged`'s own `Display` text — "an entry's stored file", "stored
    /// content", and so on.
    pub damage: String,
}

/// The result of `check_vault` (P6.0.f, Design §8.6, FR-26).
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckReport {
    /// How many entries were actually examined — equal to the vault's
    /// entry count unless `complete` is false.
    pub checked: u64,
    /// False after cancellation: a partial check is a partial answer, not
    /// a discarded one (Design §8.6).
    pub complete: bool,
    /// Every entry that failed — S-3's "named as a list", not a count.
    pub failures: Vec<CheckFailure>,
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
/// tests can drive this against [`tauri::test::MockRuntime`] instead of a
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
async fn run_blocking<T, F>(f: F) -> Result<T, ErrorInfo>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, ErrorInfo> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| internal(e.to_string()))?
}

fn summarize(vault: &Vault) -> VaultSummary {
    VaultSummary {
        entry_count: vault.entries().len() as u64,
        access: match vault.access() {
            Access::ReadWrite => "readWrite",
            Access::ReadOnly => "readOnly",
        },
        unreadable_count: vault.unreadable_entries().len() as u64,
    }
}

fn list(vault: &Vault) -> Vec<EntryInfo> {
    let unreadable = vault.unreadable_entries();
    vault
        .entries()
        .iter()
        .map(|e| EntryInfo::from_entry(e, unreadable.contains(&e.id)))
        .collect()
}

/// Opens a real vault by path and password — the GUI's equivalent of the
/// CLI's implicit open on every command (A-4).
#[tauri::command]
pub async fn open_vault<R: Runtime>(
    app: AppHandle<R>,
    path: String,
    password: String,
) -> Result<VaultSummary, ErrorInfo> {
    run_blocking(move || {
        let vault = Vault::open(std::path::Path::new(&path), &Password::new(password))?;
        let summary = summarize(&vault);
        app.state::<AppState>().set_vault(vault)?;
        Ok(summary)
    })
    .await
}

/// Creates a new vault and opens it (P6.0.b, FR-1). Always
/// `KdfParams::for_new_vaults()` — never `for_tests()`, which is compiled
/// out of release builds by design (P1.1.d) for exactly this reason.
#[tauri::command]
pub async fn create_vault<R: Runtime>(
    app: AppHandle<R>,
    path: String,
    password: String,
) -> Result<VaultSummary, ErrorInfo> {
    run_blocking(move || {
        let vault = Vault::create(
            std::path::Path::new(&path),
            &Password::new(password),
            KdfParams::for_new_vaults(),
        )?;
        let summary = summarize(&vault);
        app.state::<AppState>().set_vault(vault)?;
        Ok(summary)
    })
    .await
}

/// Debug-only: opens the fixture vault of `fixture::open` instead of a real
/// vault chosen by the user. Superseded as the app's only way in now that
/// `choose_vault_path`/`open_vault`/`create_vault` exist, but kept for the
/// fixture's other job — P6.1's rendering cases still open it deliberately.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn open_fixture_vault<R: Runtime>(app: AppHandle<R>) -> Result<VaultSummary, ErrorInfo> {
    run_blocking(move || {
        let vault = crate::fixture::open().map_err(ErrorInfo::from)?;
        let summary = summarize(&vault);
        app.state::<AppState>().set_vault(vault)?;
        Ok(summary)
    })
    .await
}

/// A native dialog for choosing where a vault is, or will be (P6.0.c,
/// Design §5, §8.1, §8.2). `mode` is `"open"` (an existing `.veil`
/// directory) or `"create"` (a new one, named here the same way a native
/// Save panel names any new file).
///
/// `"open"` picks a *file*, not a folder, despite a vault being a
/// directory on disk. `.veil` is registered as a package-conforming UTI
/// (P6.14's packaging work), which is what makes Finder show a vault as
/// one document rather than a browsable folder — but it also means macOS
/// no longer treats a `.veil` directory as a valid folder-picker
/// selection at all (the same way you pick a `.app` or `.rtfd` through an
/// Open File panel, never an Open Folder one). A folder picker here
/// stopped being able to select a vault the moment that registration
/// landed; confirmed live.
#[tauri::command]
pub async fn choose_vault_path<R: Runtime>(
    app: AppHandle<R>,
    mode: String,
) -> Result<Option<String>, ErrorInfo> {
    run_blocking(move || {
        use tauri_plugin_dialog::DialogExt;
        let chosen = match mode.as_str() {
            "open" => app
                .dialog()
                .file()
                .add_filter("Veil2 Vault", &["veil"])
                .blocking_pick_file(),
            "create" => app
                .dialog()
                .file()
                .set_file_name("Untitled.veil")
                .blocking_save_file(),
            other => {
                return Err(internal(format!(
                    "unknown choose_vault_path mode {other:?}"
                )));
            }
        };
        Ok(chosen.map(|path| path.to_string()))
    })
    .await
}

/// Every entry the open vault holds — the GUI's equivalent of `veil list`.
#[tauri::command]
pub async fn list_entries<R: Runtime>(app: AppHandle<R>) -> Result<Vec<EntryInfo>, ErrorInfo> {
    run_blocking(move || app.state::<AppState>().with_vault(|vault| Ok(list(vault)))).await
}

/// Drops the open vault from state. Nothing is written; there is nothing to
/// flush that `add`/`replace`/`delete` did not already commit.
#[tauri::command]
pub async fn close_vault<R: Runtime>(app: AppHandle<R>) -> Result<(), ErrorInfo> {
    run_blocking(move || app.state::<AppState>().clear_vault()).await
}

/// Reaches the same [`Cancel`](veil_core::Cancel) token the running
/// operation holds, kept in its own lock so this never waits behind the
/// operation it is meant to interrupt (P5.1.e).
#[tauri::command]
pub fn cancel_operation<R: Runtime>(app: AppHandle<R>) -> Result<(), ErrorInfo> {
    app.state::<AppState>().cancel_current()
}

/// Writes one entry to `destination` — the GUI's equivalent of "save a
/// copy", driven from a double-click rather than a path argument.
#[tauri::command]
pub async fn extract_entry<R: Runtime>(
    app: AppHandle<R>,
    id: u64,
    destination: String,
) -> Result<(), ErrorInfo> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let cancel = state.begin_cancellable()?;
        let mut progress = EventProgress { app: app.clone() };
        let result = state.with_vault(|vault| {
            Ok(vault.extract_to_path(
                EntryId::new(id),
                std::path::Path::new(&destination),
                &mut progress,
                &cancel,
            )?)
        });
        state.end_cancellable()?;
        result
    })
    .await
}

/// Adds every dropped path (Design §3.3, FR-9), walking a folder's contents
/// (FR-10) rather than trying to read the directory itself as a file's
/// content. A path whose name *and* folder together already match an entry
/// (never name alone — `FolderA/x` never collides with `FolderB/x`) is held
/// as a [`Collision`] for the frontend to confirm before replacing (Design
/// §8.7, §4.1), rather than failing it outright or replacing without
/// asking. One bad or colliding path never costs the rest of the batch
/// their own add.
#[tauri::command]
pub async fn add_files<R: Runtime>(
    app: AppHandle<R>,
    paths: Vec<String>,
) -> Result<AddResult, ErrorInfo> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        state.with_vault_mut(|vault| {
            let mut result = AddResult {
                added: Vec::new(),
                collisions: Vec::new(),
                failed: Vec::new(),
            };
            for path in paths {
                let source = std::path::Path::new(&path);
                if source.is_dir() {
                    add_each_in_folder(vault, source, &mut result);
                } else if let Some(name) = source.file_name().and_then(|n| n.to_str()) {
                    add_one_or_collide(vault, &path, name, "", &mut result);
                } else {
                    result.failed.push(format!("{path}: path has no file name"));
                }
            }
            Ok(result)
        })
    })
    .await
}

/// Walks `root` (FR-10) and adds each file found, folder-by-folder identity
/// preserved — a collision partway through does not abort the rest of the
/// walk the way returning early from `Vault::add_folder` would.
fn add_each_in_folder(vault: &mut Vault, root: &std::path::Path, result: &mut AddResult) {
    let found = match veil_core::vault::walk(root) {
        Ok(found) => found,
        Err(e) => {
            result.failed.push(format!("{}: {e}", root.display()));
            return;
        }
    };
    // The dropped folder's own name is part of every file's stored folder
    // (FR-10), the same as `Vault::add_folder` applies it (this function's
    // own doc comment says why that method is not called directly).
    // Without it, a file at the root of one dropped folder and a
    // same-named file at the root of a *different* dropped folder would
    // both land at the vault's root — indistinguishable identities for two
    // files that are not the same file.
    let root_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    for file in found.files {
        let path = file.path.to_string_lossy().into_owned();
        let folder = if file.folder.is_empty() {
            root_name.to_owned()
        } else {
            format!("{root_name}/{}", file.folder)
        };
        add_one_or_collide(vault, &path, &file.name, &folder, result);
    }
    for skipped in found.skipped {
        let reason = match skipped.reason {
            veil_core::vault::SkipReason::SymbolicLink => "a link, and links are not followed",
            veil_core::vault::SkipReason::NotARegularFile => "not a regular file",
        };
        result
            .failed
            .push(format!("{}: {reason}", skipped.path.display()));
    }
}

fn add_one_or_collide(
    vault: &mut Vault,
    path: &str,
    name: &str,
    folder: &str,
    result: &mut AddResult,
) {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) => {
            result.failed.push(format!("{path}: {e}"));
            return;
        }
    };
    match vault.add(
        name,
        folder,
        &mut file,
        &mut veil_core::NoProgress,
        &veil_core::Cancel::new(),
    ) {
        Ok(id) => {
            if let Some(entry) = vault.entries().iter().find(|e| e.id == id) {
                result.added.push(EntryInfo::from_entry(entry, false));
            }
        }
        Err(veil_core::Error::AlreadyExists) => {
            result.collisions.push(Collision {
                path: path.to_owned(),
                name: name.to_owned(),
                folder: folder.to_owned(),
            });
        }
        Err(e) => result.failed.push(format!("{path}: {e}")),
    }
}

/// Replaces one entry's content in place (P6.16, Design §8.7, FR-13). The
/// entry's identity — folder and name — does not change; its content and
/// its recorded "added" time do.
#[tauri::command]
pub async fn replace_entry<R: Runtime>(
    app: AppHandle<R>,
    folder: String,
    name: String,
    source_path: String,
) -> Result<EntryInfo, ErrorInfo> {
    run_blocking(move || {
        app.state::<AppState>().with_vault_mut(|vault| {
            let mut file = std::fs::File::open(std::path::Path::new(&source_path))
                .map_err(|e| internal(e.to_string()))?;
            let id = vault.replace(
                &folder,
                &name,
                &mut file,
                &mut veil_core::NoProgress,
                &veil_core::Cancel::new(),
            )?;
            vault
                .entries()
                .iter()
                .find(|e| e.id == id)
                .map(|e| EntryInfo::from_entry(e, false))
                .ok_or_else(|| internal("replaced entry not found in its own vault"))
        })
    })
    .await
}

/// Removes an entry, immediately (P6.0.e, FR-22).
#[tauri::command]
pub async fn delete_entry<R: Runtime>(app: AppHandle<R>, id: u64) -> Result<(), ErrorInfo> {
    run_blocking(move || {
        app.state::<AppState>()
            .with_vault_mut(|vault| Ok(vault.delete(EntryId::new(id))?))
    })
    .await
}

/// Changes the vault's password (P6.17, HC-5). Requires the current
/// password even though the vault is already unlocked — `veil-core`'s API
/// requires it, deliberately, and this command does not work around that.
#[tauri::command]
pub async fn change_password<R: Runtime>(
    app: AppHandle<R>,
    current: String,
    new: String,
) -> Result<(), ErrorInfo> {
    run_blocking(move || {
        app.state::<AppState>().with_vault_mut(|vault| {
            Ok(vault.change_password(
                &Password::new(current),
                &Password::new(new),
                KdfParams::for_new_vaults(),
            )?)
        })
    })
    .await
}

/// Verifies the whole vault (P6.0.f, Design §8.6, FR-26). Progress and
/// cancellation reuse `extract_entry`'s event channel and cancel token.
#[tauri::command]
pub async fn check_vault<R: Runtime>(app: AppHandle<R>) -> Result<CheckReport, ErrorInfo> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let cancel = state.begin_cancellable()?;
        let mut progress = EventProgress { app: app.clone() };
        let result = state.with_vault(|vault| {
            let report = vault.verify(&mut progress, &cancel)?;
            let failures = report
                .verdicts
                .iter()
                .filter_map(|v| {
                    let Outcome::Failed(damage) = v.outcome else {
                        return None;
                    };
                    let entry = vault.entries().iter().find(|e| e.id == v.id)?;
                    Some(CheckFailure {
                        id: entry.id.get(),
                        name: entry.name.clone(),
                        folder: entry.folder.clone(),
                        damage: damage.to_string(),
                    })
                })
                .collect();
            Ok(CheckReport {
                checked: report.verdicts.len() as u64,
                complete: report.complete,
                failures,
            })
        });
        state.end_cancellable()?;
        result
    })
    .await
}

/// A native save dialog for an extraction destination (Design §3.3, FR-17).
#[tauri::command]
pub async fn choose_save_path(
    app: AppHandle,
    suggested_name: String,
) -> Result<Option<String>, ErrorInfo> {
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

/// A native dialog for choosing a source file — replace's and the explicit
/// add control's equivalent of the drop target (Design §8.3, §8.7).
#[tauri::command]
pub async fn choose_source_paths(app: AppHandle, multiple: bool) -> Result<Vec<String>, ErrorInfo> {
    run_blocking(move || {
        use tauri_plugin_dialog::DialogExt;
        let paths = if multiple {
            app.dialog()
                .file()
                .blocking_pick_files()
                .unwrap_or_default()
        } else {
            app.dialog()
                .file()
                .blocking_pick_file()
                .into_iter()
                .collect()
        };
        Ok(paths.into_iter().map(|p| p.to_string()).collect())
    })
    .await
}
