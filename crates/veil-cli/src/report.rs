//! The commands whose result is a listing rather than a change: what a vault
//! holds, what it takes up, and whether it is intact (FR-7, FR-8, FR-26).

use std::path::PathBuf;

use veil_core::Damaged;
use veil_core::index::Entry;
use veil_core::vault::Vault;
use veil_core::vault::{Outcome, SkipReason, Skipped};

use crate::cli::{Format, GroupBy};
use crate::failure::{Failure, Run};
use crate::output::{self, FileRow};
use crate::progress::{Stderr, cancel_on_interrupt};

/// One path a folder add declined, and why (FR-11).
#[derive(Debug, serde::Serialize)]
pub struct SkippedRow {
    /// The path, as it was encountered.
    pub path: String,
    /// Why it was not added.
    pub reason: &'static str,
}

impl From<&Skipped> for SkippedRow {
    fn from(skipped: &Skipped) -> Self {
        Self {
            path: skipped.path.display().to_string(),
            reason: match skipped.reason {
                SkipReason::SymbolicLink => "a link, and links are not followed",
                SkipReason::NotARegularFile => "not a regular file",
            },
        }
    }
}

/// Lists the files a vault holds, filtered and optionally grouped (FR-7,
/// FR-8, FR-29).
pub fn list(
    vault: &Vault,
    folder: Option<&str>,
    name: Option<&str>,
    group: Option<GroupBy>,
    format: Format,
) -> Run<()> {
    let mut rows: Vec<FileRow> = vault
        .entries()
        .iter()
        .filter(|e| folder.is_none_or(|f| e.folder.starts_with(f)))
        .filter(|e| name.is_none_or(|n| e.name.contains(n)))
        .map(FileRow::from)
        .collect();
    rows.sort_by(|a, b| (&a.folder, &a.name).cmp(&(&b.folder, &b.name)));

    match (format, group) {
        (Format::Json, None) => output::json(&serde_json::json!({ "files": rows })),
        (Format::Json, Some(by)) => {
            output::json(&serde_json::json!({ "groups": grouped_json(&rows, by) }))
        }
        (Format::Table, None) => output::table(&rows),
        (Format::Table, Some(by)) => output::grouped(&rows, by),
    }
}

/// The JSON shape of a grouped listing: one object per group, each carrying
/// the group's own key — a folder string, or an extension string that is
/// `null` for the reserved no-extension bucket (FR-29) — and its files, built
/// from the same [`output::group_key`] the table view groups by, so the two
/// output modes cannot disagree about what a group is.
fn grouped_json(rows: &[FileRow], by: GroupBy) -> Vec<serde_json::Value> {
    let mut keys: Vec<Option<String>> = rows.iter().map(|r| output::group_key(r, by)).collect();
    keys.sort();
    keys.dedup();

    keys.into_iter()
        .map(|key| {
            let files: Vec<&FileRow> = rows
                .iter()
                .filter(|r| output::group_key(r, by) == key)
                .collect();
            serde_json::json!({ "group": key, "files": files })
        })
        .collect()
}

/// One file's complete recorded metadata (FR-28) — a superset of `FileRow`.
///
/// No content hash: Design §8.9 keeps it off the GUI's own detail panel, and
/// that decision is held here too, so the two peers agree on what "detail"
/// means (A-4).
#[derive(Debug, serde::Serialize)]
struct DetailInfo {
    name: String,
    folder: String,
    size: u64,
    /// The source file's own modification time, from before it was added.
    modified: u64,
    /// When it was added to the vault (or last replaced).
    added: u64,
}

/// Reports everything FR-28 covers for one file. Requires no content read:
/// every field comes from the resident index, the same as `list`'s.
pub fn detail(entry: &Entry, format: Format) -> Run<()> {
    let info = DetailInfo {
        name: entry.name.clone(),
        folder: entry.folder.clone(),
        size: entry.size,
        modified: entry.source_mtime,
        added: entry.added_at,
    };

    match format {
        Format::Json => output::json(&info),
        Format::Table => output::say(
            format,
            &format!(
                "Name      {}\n\
                 Folder    {}\n\
                 Size      {} bytes\n\
                 Modified  {}\n\
                 Added     {}",
                info.name,
                info.folder,
                output::count(info.size),
                output::stamp(info.modified),
                output::stamp(info.added),
            ),
        ),
    }
}

/// Reports what a vault holds and what it stores (FR-7).
///
/// Derived from the resident entry list, the same way `statistics()` always
/// is — there is no separate figure this command walks the vault to measure.
pub fn info(vault: &Vault, format: Format) -> Run<()> {
    let stats = vault.statistics();

    match format {
        Format::Json => output::json(&serde_json::json!({
            "files": stats.entry_count,
            "logical_bytes": stats.logical_bytes,
        })),
        Format::Table => output::say(
            format,
            &format!(
                "Files    {:>12}\n\
                 Stored   {:>12}",
                output::count(stats.entry_count),
                output::human_size(stats.logical_bytes),
            ),
        ),
    }
}

/// Reads and authenticates everything, and reports every file that fails
/// (FR-26, S-3).
///
/// Exits non-zero when anything failed, so a backup script can use it as a
/// check without parsing what it printed (Spec §5.2).
pub fn check(vault: &Vault, format: Format) -> Run<()> {
    let mut progress = Stderr::new("checking");
    let report = vault.verify(&mut progress, &cancel_on_interrupt())?;
    progress.finish();

    let failures: Vec<serde_json::Value> = report
        .verdicts
        .iter()
        .filter_map(|verdict| match verdict.outcome {
            Outcome::Passed => None,
            Outcome::Failed(what) => {
                let entry = vault.entries().iter().find(|e| e.id == verdict.id);
                Some(serde_json::json!({
                    "name": entry.map_or_else(String::new, |e| e.name.clone()),
                    "folder": entry.map_or_else(String::new, |e| e.folder.clone()),
                    "damage": describe(what),
                }))
            }
        })
        .collect();

    let checked = report.verdicts.len();
    if format == Format::Json {
        output::json(&serde_json::json!({
            "checked": checked,
            "complete": report.complete,
            "damaged": failures,
        }))?;
    } else if failures.is_empty() {
        output::say(
            format,
            &format!(
                "Checked {} file{}. No damage found.{}",
                output::count(checked as u64),
                output::plural(checked),
                if report.complete {
                    ""
                } else {
                    "\nThe check was stopped early, so this covers only what it reached."
                }
            ),
        )?;
    } else {
        let mut text = format!(
            "{} of {} file{} damaged:\n",
            output::count(failures.len() as u64),
            output::count(checked as u64),
            output::plural(checked),
        );
        for failure in &failures {
            text.push_str(&format!(
                "  {}{}  —  {}\n",
                folder_prefix(failure),
                string(failure, "name"),
                string(failure, "damage")
            ));
        }
        // S-3: the next thing this person does is decide whether to go looking
        // for a backup, and that decision needs the plain fact.
        text.push_str("\nVeil2 cannot repair these. Restore them from a backup if you have one.");
        output::say(format, &text)?;
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(Failure::Damage(format!(
            "{} of {checked} files are damaged",
            failures.len()
        )))
    }
}

/// Reports what an add stored and what it declined (FR-9, FR-10, FR-11, FR-27).
pub fn added(
    rows: &[FileRow],
    skipped: &[SkippedRow],
    sources: &[PathBuf],
    format: Format,
) -> Run<()> {
    if format == Format::Json {
        return output::json(&serde_json::json!({
            "added": rows,
            "skipped": skipped,
            "originals_kept": true,
        }));
    }

    output::table(rows)?;
    if !skipped.is_empty() {
        let mut text = format!(
            "\nSkipped {} path{}:",
            output::count(skipped.len() as u64),
            output::plural(skipped.len())
        );
        for item in skipped {
            text.push_str(&format!("\n  {}  —  {}", item.path, item.reason));
        }
        output::say(format, &text)?;
    }

    // FR-27, at the moment it happens: an unprotected copy the user has
    // forgotten about is the likeliest route by which data leaves Veil2.
    let where_from = sources
        .iter()
        .map(|s| s.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    output::say(
        format,
        &format!("\nThe originals are still at {where_from}. Veil2 did not move or delete them."),
    )
}

fn describe(what: Damaged) -> String {
    what.to_string()
}

fn string(value: &serde_json::Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_owned()
}

fn folder_prefix(value: &serde_json::Value) -> String {
    let folder = string(value, "folder");
    if folder.is_empty() {
        folder
    } else {
        format!("{folder}/")
    }
}
