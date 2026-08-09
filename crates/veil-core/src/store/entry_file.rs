//! Entry files (Spec §4.1, §4.5).
//!
//! Each entry is its own file under `entries/`, named by its id. Reading one
//! entry opens only that file (A-5); damage to one entry's file cannot spread
//! past it (S-3), so there is no attribution to compute here — a missing or
//! unreadable file already names the entry it belongs to.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::index::EntryId;

/// Directory holding entry files, relative to the vault.
pub const ENTRIES_DIR: &str = "entries";

/// Path of one entry's file.
#[must_use]
pub fn entry_path(vault_dir: &Path, id: EntryId) -> PathBuf {
    vault_dir
        .join(ENTRIES_DIR)
        .join(format!("{:08}.entry", id.get()))
}

/// Whether an entry's file exists. Used for damage attribution — one
/// existence check, no content read, so a vault with a missing file opens at
/// the same speed as one without (S-2).
#[must_use]
pub fn exists(vault_dir: &Path, id: EntryId) -> bool {
    entry_path(vault_dir, id).exists()
}

/// Opens an entry's file for reading.
///
/// # Errors
///
/// [`crate::Error::Io`] if the file cannot be opened, including if it is
/// missing — the caller distinguishes a missing file (damage to this one
/// entry, §4.5) from any other I/O failure.
pub fn open_for_read(vault_dir: &Path, id: EntryId) -> Result<fs::File> {
    Ok(fs::File::open(entry_path(vault_dir, id))?)
}

/// Removes an entry's file and makes the removal durable.
///
/// A file already gone frees nothing and is not an error — delete and replace
/// both call this after their index commit, and a crash between the two
/// leaves nothing here to remove (HC-4).
///
/// # Errors
///
/// [`crate::Error::Io`] if the file cannot be removed or the directory cannot
/// be synced.
pub fn remove(vault_dir: &Path, id: EntryId) -> Result<()> {
    let path = entry_path(vault_dir, id);
    if fs::metadata(&path).is_ok() {
        fs::remove_file(&path)?;
    }
    crate::durable::sync_dir(&vault_dir.join(ENTRIES_DIR))?;
    Ok(())
}

/// A sink for one entry's content. Implements [`Write`], so encryption
/// streams straight into it.
///
/// There is no rollback. A cancelled or failed ingest simply stops writing
/// and never commits an index generation naming this file — the file itself
/// is left as unreferenced residue (Spec §4.5), not undone. Nothing sweeps
/// it, by decision: an index that is momentarily behind its own directory is
/// indistinguishable from this case by construction, and removing a file on
/// that guess risks the loss HC-4 forbids.
pub struct EntryWriter {
    file: fs::File,
    dir: PathBuf,
}

impl EntryWriter {
    /// Creates the entry's file, ready to be written from its first byte.
    ///
    /// # Errors
    ///
    /// [`crate::Error::Io`] if the `entries/` directory cannot be created or
    /// the file cannot be created.
    pub fn create(vault_dir: &Path, id: EntryId) -> Result<Self> {
        let dir = vault_dir.join(ENTRIES_DIR);
        fs::create_dir_all(&dir)?;
        let path = entry_path(vault_dir, id);
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        Ok(Self { file, dir })
    }

    /// Fsyncs the file, then the containing directory — the ordering FR-12
    /// depends on. Content is durable before anything may refer to it.
    ///
    /// # Errors
    ///
    /// [`crate::Error::Io`] if either sync fails.
    pub fn finish(self) -> Result<()> {
        self.file.sync_all()?;
        crate::durable::sync_dir(&self.dir)?;
        Ok(())
    }
}

impl Write for EntryWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.file.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
