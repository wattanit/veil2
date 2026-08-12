//! The command surface (Spec §5.2; Design §3.4, §7).
//!
//! Every user-facing word here comes from the Design Guideline's vocabulary
//! table: *file*, *folder*, *add*, *save a copy*, *lock*, *check for damage*.
//! The words this repository uses internally — entry, ingest, extract, verify —
//! stop at the process boundary.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Printed under `--help`. A script depending on these numbers cannot be
/// expected to find them by experiment (Spec §5.2).
pub const EXIT_CODES: &str = "\
Exit codes:
  0   done
  1   something unexpected failed
  2   the command was used wrongly, or the password was too short
  3   wrong password
  4   not a vault, or a vault this version cannot read
  5   damage found
  6   the vault is open somewhere else
  7   the vault changed on disk
  8   the vault is read-only
  9   a limit would be exceeded
  10  cancelled
  11  the vault's storage went away
  12  a password was needed and there was no way to ask for one
  13  no file at that path, or a file is already there

The password is never taken as an argument: arguments are visible in process
listings and shell history. Use --password-file, or the VEIL_PASSWORD
environment variable, or type it when asked.";

/// How results are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// A table to read.
    Table,
    /// JSON to script against.
    Json,
}

/// `veil` — keep files in an encrypted vault.
#[derive(Debug, Parser)]
#[command(name = "veil", version, about, after_help = EXIT_CODES)]
pub struct Cli {
    /// How to write results.
    #[arg(long, global = true, value_enum, default_value = "table")]
    pub format: Format,

    /// Read the vault's password from this file. One trailing newline is
    /// ignored; nothing else is trimmed.
    #[arg(long, global = true, value_name = "FILE")]
    pub password_file: Option<PathBuf>,

    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

/// One command per thing the vault can do.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Make a new vault.
    Create {
        /// Where to make it.
        vault: PathBuf,
    },

    /// Add files to a vault.
    Add {
        /// The vault.
        vault: PathBuf,
        /// Files to add. A folder adds every file beneath it, recording each
        /// one's folder as you had it. Links are not followed.
        #[arg(required = true)]
        sources: Vec<PathBuf>,
        /// Folder to record the added files under. Applies to single files
        /// only — a folder brings its own.
        #[arg(long)]
        folder: Option<String>,
    },

    /// List the files in a vault.
    List {
        /// The vault.
        vault: PathBuf,
        /// Show only files whose folder starts with this.
        #[arg(long)]
        folder: Option<String>,
        /// Show only files whose name contains this.
        #[arg(long)]
        name: Option<String>,
        /// Group the listing by folder.
        #[arg(long)]
        group: bool,
    },

    /// Show everything recorded about one file.
    Detail {
        /// The vault.
        vault: PathBuf,
        /// The file, as folder and name together: work/2024/report.pdf
        file: String,
    },

    /// Save an unprotected copy of one file out of a vault.
    SaveCopy {
        /// The vault.
        vault: PathBuf,
        /// The file, as folder and name together: work/2024/report.pdf
        file: String,
        /// Where to write the copy.
        #[arg(long, value_name = "DESTINATION")]
        to: PathBuf,
        /// Overwrite whatever is already at the destination.
        #[arg(long)]
        force: bool,
    },

    /// Replace a file in a vault with new content.
    Replace {
        /// The vault.
        vault: PathBuf,
        /// The file to replace, as folder and name together.
        file: String,
        /// The new content.
        #[arg(long, value_name = "SOURCE")]
        from: PathBuf,
    },

    /// Delete a file from a vault.
    Delete {
        /// The vault.
        vault: PathBuf,
        /// The file, as folder and name together.
        file: String,
    },

    /// Check a vault for damage. Reads everything; writes nothing.
    Check {
        /// The vault.
        vault: PathBuf,
    },

    /// Show what a vault holds and what it takes up.
    Info {
        /// The vault.
        vault: PathBuf,
    },

    /// Change a vault's password.
    Password {
        /// The vault.
        vault: PathBuf,
        /// Read the new password from this file.
        #[arg(long, value_name = "FILE")]
        new_password_file: Option<PathBuf>,
    },
}
