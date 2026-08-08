//! What each command does (Spec §5.2).
//!
//! The honesty clauses FR-29 requires are attached here, at the moments that
//! produce them: unrecoverability before a vault exists, the retained original
//! after adding, the persistence of deleted bytes, the unprotected copy.

use std::path::{Path, PathBuf};

use veil_core::Error;
use veil_core::crypto::KdfParams;
use veil_core::index::{Entry, EntryId};
use veil_core::store::DEFAULT_PACK_CAP;
use veil_core::vault::Vault;

use crate::cli::{Cli, Command, Format};
use crate::failure::{Failure, Run};
use crate::output::{self, FileRow};
use crate::password::{self, NEW_PASSWORD_ENV, PASSWORD_ENV};
use crate::progress::{Stderr, cancel_on_interrupt};
use crate::report;

/// Runs the parsed command.
pub fn dispatch(cli: &Cli) -> Run<()> {
    match &cli.command {
        Command::Create { vault } => create(vault, cli),
        Command::Add {
            vault,
            sources,
            folder,
        } => add(vault, sources, folder.as_deref(), cli),
        Command::List {
            vault,
            folder,
            name,
            group,
        } => report::list(
            &open(vault, cli)?,
            folder.as_deref(),
            name.as_deref(),
            *group,
            cli.format,
        ),
        Command::SaveCopy {
            vault,
            file,
            to,
            force,
        } => save_copy(vault, file, to, *force, cli),
        Command::Replace { vault, file, from } => replace(vault, file, from, cli),
        Command::Delete { vault, file } => delete(vault, file, cli),
        Command::Check { vault } => report::check(&open(vault, cli)?, cli.format),
        Command::Info { vault } => report::info(&open(vault, cli)?, cli.format),
        Command::Password {
            vault,
            new_password_file,
        } => change_password(vault, new_password_file.as_deref(), cli),
    }
}

/// Opens a vault with the password from wherever it is available.
fn open(dir: &Path, cli: &Cli) -> Run<Vault> {
    let secret = password::resolve(
        cli.password_file.as_deref(),
        PASSWORD_ENV,
        "Password",
        false,
    )?;
    Ok(Vault::open(dir, &secret)?)
}

fn create(dir: &Path, cli: &Cli) -> Run<()> {
    // Before the vault exists, not after (HC-7, FR-29). A warning that arrives
    // once the password is already set is a warning about a decision already
    // made.
    output::say(
        cli.format,
        "A lost password cannot be recovered. There is no reset, no backup key, and\n\
         no way for anyone to get in without it. If you forget it, everything in this\n\
         vault is gone for good.\n",
    )?;

    let secret = password::resolve(
        cli.password_file.as_deref(),
        PASSWORD_ENV,
        "Password for the new vault",
        true,
    )?;

    let vault = Vault::create(dir, &secret, KdfParams::for_new_vaults(), DEFAULT_PACK_CAP)?;
    vault.lock();

    match cli.format {
        Format::Table => output::say(cli.format, &format!("Made a vault at {}", dir.display())),
        Format::Json => output::json(&serde_json::json!({ "vault": dir.display().to_string() })),
    }
}

fn add(dir: &Path, sources: &[PathBuf], folder: Option<&str>, cli: &Cli) -> Run<()> {
    let mut vault = open(dir, cli)?;
    let cancel = cancel_on_interrupt();
    let mut added: Vec<FileRow> = Vec::new();
    let mut skipped: Vec<report::SkippedRow> = Vec::new();

    for source in sources {
        let mut progress = Stderr::new("adding");
        if source.is_dir() {
            if folder.is_some() {
                return Err(Failure::Usage(
                    "--folder applies to single files; a folder brings its own".to_owned(),
                ));
            }
            let outcome = vault
                .add_folder(source, &mut progress, &cancel)
                .map_err(|e| already_there(e, &source.display().to_string()))?;
            progress.finish();
            added.extend(rows_for(&vault, &outcome.added));
            skipped.extend(outcome.skipped.iter().map(report::SkippedRow::from));
        } else {
            let id = vault
                .add_path(source, folder.unwrap_or(""), &mut progress, &cancel)
                .map_err(|e| already_there(e, &source.display().to_string()))?;
            progress.finish();
            added.extend(rows_for(&vault, std::slice::from_ref(&id)));
        }
    }

    report::added(&added, &skipped, sources, cli.format)
}

fn save_copy(dir: &Path, file: &str, to: &Path, force: bool, cli: &Cli) -> Run<()> {
    let vault = open(dir, cli)?;
    let id = locate(&vault, file)?;

    // `--to a-folder` means "into it", which is what anyone typing it expects.
    let destination = if to.is_dir() {
        to.join(split(file).1)
    } else {
        to.to_path_buf()
    };

    // FR-18: named, and never silent. The original Veil overwrote quietly, and
    // a failed save destroyed the user's only good copy.
    if destination.exists() && !force {
        return Err(Failure::Usage(format!(
            "{} is already there. Pass --force to overwrite it",
            destination.display()
        )));
    }

    let mut progress = Stderr::new("saving");
    let outcome = vault.extract_to_path(id, &destination, &mut progress, &cancel_on_interrupt());
    progress.finish();
    outcome?;

    match cli.format {
        Format::Table => output::say(
            cli.format,
            &format!(
                "Saved a copy to {}\nThat copy is an ordinary file now. Nothing protects it.",
                destination.display()
            ),
        ),
        Format::Json => output::json(&serde_json::json!({
            "saved": destination.display().to_string(),
            "protected": false,
        })),
    }
}

fn replace(dir: &Path, file: &str, from: &Path, cli: &Cli) -> Run<()> {
    let mut vault = open(dir, cli)?;
    let (folder, name) = split(file);
    if vault.find(folder, name).is_none() {
        return Err(Failure::NoSuchFile(no_such(file)));
    }

    let mut source = std::fs::File::open(from)
        .map_err(|e| Failure::Usage(format!("cannot read {}: {e}", from.display())))?;
    let mut progress = Stderr::new("replacing");
    let outcome = vault.replace(
        folder,
        name,
        &mut source,
        &mut progress,
        &cancel_on_interrupt(),
    );
    progress.finish();
    outcome?;

    match cli.format {
        Format::Table => output::say(
            cli.format,
            &format!(
                "Replaced {file}\nThe file you replaced it from is still at {}. \
                 Veil2 did not move or delete it.",
                from.display()
            ),
        ),
        Format::Json => output::json(&serde_json::json!({ "replaced": file })),
    }
}

fn delete(dir: &Path, file: &str, cli: &Cli) -> Run<()> {
    let mut vault = open(dir, cli)?;
    let id = locate(&vault, file)?;
    vault.delete(id)?;

    match cli.format {
        Format::Table => output::say(
            cli.format,
            &format!(
                "Deleted {file}\nIts stored bytes stay in the vault until space is reclaimed. \
                 This version\ncannot reclaim space yet, so anyone with this vault could still \
                 recover them.",
            ),
        ),
        Format::Json => output::json(&serde_json::json!({
            "deleted": file,
            "bytes_still_stored": true,
        })),
    }
}

fn change_password(dir: &Path, new_file: Option<&Path>, cli: &Cli) -> Run<()> {
    let mut vault = open(dir, cli)?;
    let old = password::resolve(
        cli.password_file.as_deref(),
        PASSWORD_ENV,
        "Current password",
        false,
    )?;
    let new = password::resolve(new_file, NEW_PASSWORD_ENV, "New password", true)?;

    vault.change_password(&old, &new, KdfParams::for_new_vaults())?;

    match cli.format {
        Format::Table => output::say(
            cli.format,
            "Changed the password. Nothing else about the vault changed, and the old\n\
             password no longer opens it.",
        ),
        Format::Json => output::json(&serde_json::json!({ "password_changed": true })),
    }
}

/// Splits a stored path into the folder and name that identify a file (FR-13).
fn split(file: &str) -> (&str, &str) {
    file.rsplit_once('/').map_or(("", file), |(f, n)| (f, n))
}

/// Finds the one file at a path.
///
/// The two-or-more arm is a guard rather than a reachable case: FR-34 leaves no
/// way to store a second file at one path. It stays because the alternative, if
/// it is ever reached, is deleting an arbitrary one of two files.
fn locate(vault: &Vault, file: &str) -> Run<EntryId> {
    let (folder, name) = split(file);
    let matches: Vec<&Entry> = vault
        .entries()
        .iter()
        .filter(|e| e.folder == folder && e.name == name)
        .collect();

    match matches.as_slice() {
        [one] => Ok(one.id),
        [] => Err(Failure::NoSuchFile(no_such(file))),
        many => Err(Failure::Usage(format!(
            "{file} matches {} files in this vault, so it does not say which one you mean",
            many.len()
        ))),
    }
}

fn no_such(file: &str) -> String {
    format!("this vault holds no file at {file}")
}

/// Names the path in FR-34's refusal. The library will not hold a name (HC-1),
/// so the naming happens here, where the name came from.
fn already_there(error: Error, source: &str) -> Failure {
    match error {
        Error::AlreadyExists => Failure::AlreadyThere(format!(
            "this vault already holds a file at that path, so {source} was not added. \
             Use `veil replace` to put new content there"
        )),
        other => Failure::Vault(other),
    }
}

/// The rows for a set of just-added files, read back from the index so the
/// listing shows what was stored rather than what was asked for.
fn rows_for(vault: &Vault, ids: &[EntryId]) -> Vec<FileRow> {
    vault
        .entries()
        .iter()
        .filter(|e| ids.contains(&e.id))
        .map(FileRow::from)
        .collect()
}

impl From<&Entry> for FileRow {
    fn from(entry: &Entry) -> Self {
        Self {
            name: entry.name.clone(),
            folder: entry.folder.clone(),
            size: entry.size,
            added: entry.added_at,
        }
    }
}
