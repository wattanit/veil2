//! The commands whose result is a listing rather than a change: what a vault
//! holds, what it takes up, and whether it is intact (FR-6, FR-7, FR-8, FR-33).

use std::path::PathBuf;

use veil_core::Damaged;
use veil_core::vault::Vault;
use veil_core::vault::{Outcome, SkipReason, Skipped};

use crate::cli::Format;
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

/// Lists the files a vault holds, filtered and optionally grouped (FR-6, FR-7).
pub fn list(
    vault: &Vault,
    folder: Option<&str>,
    name: Option<&str>,
    group: bool,
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
        (Format::Json, _) => output::json(&serde_json::json!({ "files": rows })),
        (Format::Table, true) => output::grouped(&rows),
        (Format::Table, false) => output::table(&rows),
    }
}

/// Reports what a vault holds and what it occupies (FR-8, FR-22).
pub fn info(vault: &Vault, format: Format) -> Run<()> {
    let stats = vault.statistics();
    let share = if stats.physical_bytes == 0 {
        0.0
    } else {
        stats.reclaimable_bytes as f64 / stats.physical_bytes as f64 * 100.0
    };

    match format {
        Format::Json => output::json(&serde_json::json!({
            "files": stats.entry_count,
            "logical_bytes": stats.logical_bytes,
            "physical_bytes": stats.physical_bytes,
            "reclaimable_bytes": stats.reclaimable_bytes,
            "reclaimable_share": share,
        })),
        Format::Table => output::say(
            format,
            &format!(
                "Files         {:>12}\n\
                 Stored        {:>12}\n\
                 On disk       {:>12}\n\
                 Reclaimable   {:>12}  ({share:.1}%)",
                output::count(stats.entry_count),
                output::human_size(stats.logical_bytes),
                output::human_size(stats.physical_bytes),
                output::human_size(stats.reclaimable_bytes),
            ),
        ),
    }
}

/// Reads and authenticates everything, and reports every file that fails
/// (FR-33, S-4).
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
        // S-4: the next thing this person does is decide whether to go looking
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

/// Reports what an add stored and what it declined (FR-9, FR-10, FR-11, FR-29).
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

    // FR-29, at the moment it happens: an unprotected copy the user has
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
