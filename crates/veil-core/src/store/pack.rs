//! Pack files and extents (Spec §4.5; S-3, S-4, A-5, FR-25).
//!
//! Append-only and capped. The cap buys three things: a change dirties one pack
//! plus the index, so sync transfers bytes proportional to the change (S-3);
//! damage costs only the entries with extents in that pack, and extents make
//! those entries enumerable (S-4); and compaction rewrites one pack at a time,
//! so it needs about one pack of working space (FR-25).
//!
//! The cap is a parameter, not a constant — a multi-pack test needing gigabytes
//! of fixture gets marked ignored and stops running.

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::index::{Entry, EntryId, Extent};

/// Default maximum bytes in one pack file. Initial; tunable. Smaller improves
/// sync granularity (S-3) and damage locality (S-4).
pub const DEFAULT_PACK_CAP: u64 = 1024 * 1024 * 1024;

/// Directory holding pack files, relative to the vault.
pub const PACKS_DIR: &str = "packs";

/// Path of one pack file.
#[must_use]
pub fn pack_path(vault_dir: &Path, pack_id: u32) -> PathBuf {
    vault_dir.join(PACKS_DIR).join(format!("{pack_id:06}.pack"))
}

/// Every pack id present on disk, ascending.
///
/// # Errors
///
/// [`Error::Io`] if the packs directory cannot be listed.
pub fn existing_pack_ids(vault_dir: &Path) -> Result<Vec<u32>> {
    let dir = vault_dir.join(PACKS_DIR);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut ids = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "pack")
            && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            && let Ok(id) = stem.parse::<u32>()
        {
            ids.push(id);
        }
    }
    ids.sort_unstable();
    Ok(ids)
}

/// Total bytes held by every pack file on disk. A diagnostic and a test oracle,
/// never a source for the statistics of FR-22 — those are incremental, and this
/// is what they get checked against.
///
/// # Errors
///
/// [`Error::Io`] if the packs directory cannot be listed.
pub fn total_pack_bytes(vault_dir: &Path) -> Result<u64> {
    let mut total = 0;
    for id in existing_pack_ids(vault_dir)? {
        total += fs::metadata(pack_path(vault_dir, id))?.len();
    }
    Ok(total)
}

/// Removes one pack file and makes the removal durable, returning the bytes it
/// held.
///
/// Removing the file is a change to the directory, so the directory is what has
/// to be synced; syncing the file would be syncing something that no longer
/// exists. A pack that is already gone frees nothing and is not an error — both
/// reclaiming space and reconciliation can arrive here after a crash that got
/// part-way (HC-4).
///
/// # Errors
///
/// [`Error::Io`] if the file cannot be removed or the directory cannot be
/// synced.
pub fn remove_pack(vault_dir: &Path, pack_id: u32) -> Result<u64> {
    let path = pack_path(vault_dir, pack_id);
    let Ok(metadata) = fs::metadata(&path) else {
        return Ok(0);
    };
    let length = metadata.len();
    fs::remove_file(&path)?;
    crate::durable::sync_dir(&vault_dir.join(PACKS_DIR))?;
    Ok(length)
}

/// Every entry with at least one extent in the given pack — the attribution
/// S-4 requires (a list of unreadable files, not a failed vault).
#[must_use]
pub fn entries_in_pack(entries: &[Entry], pack_id: u32) -> Vec<EntryId> {
    entries
        .iter()
        .filter(|e| e.extents.iter().any(|x| x.pack_id == pack_id))
        .map(|e| e.id)
        .collect()
}

/// An append-only sink that rolls over at the cap, recording where everything
/// landed. Implements [`Write`], so encryption streams straight into it.
pub struct PackSink<'a> {
    vault_dir: &'a Path,
    cap: u64,
    pack_id: u32,
    offset: u64,
    file: Option<fs::File>,
    extents: Vec<Extent>,
    /// Where the sink started, so an abandoned write can be undone exactly.
    start_pack_id: u32,
    start_offset: u64,
    /// Whether this sink brought a pack file into existence. A new file needs
    /// its directory synced too, or its bytes are durable under no name.
    named_a_pack: bool,
    /// Set by [`PackSink::seal_extent`]; stops the next write from merging into
    /// the extent before it.
    break_extent: bool,
}

impl<'a> PackSink<'a> {
    /// Opens a sink that appends to the vault's newest pack, or starts one.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the packs directory cannot be created or listed.
    pub fn open(vault_dir: &'a Path, cap: u64) -> Result<Self> {
        fs::create_dir_all(vault_dir.join(PACKS_DIR))?;
        let ids = existing_pack_ids(vault_dir)?;

        let (pack_id, offset) = match ids.last() {
            Some(&id) => {
                let len = fs::metadata(pack_path(vault_dir, id))?.len();
                if len >= cap { (id + 1, 0) } else { (id, len) }
            }
            None => (1, 0),
        };

        Ok(Self {
            vault_dir,
            cap,
            pack_id,
            offset,
            file: None,
            extents: Vec::new(),
            start_pack_id: pack_id,
            start_offset: offset,
            named_a_pack: false,
            break_extent: false,
        })
    }

    /// Opens a sink that starts a pack of its own rather than appending to the
    /// newest, so its bytes are separable from everything already stored.
    ///
    /// This is what reclaiming space writes into: the live extents of one pack
    /// are copied here, and the new pack takes an identifier above every pack
    /// present. Identifiers are therefore never reused — the new pack is
    /// created before the old one is removed, so the highest only ever rises.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the packs directory cannot be created or listed.
    pub fn open_fresh(vault_dir: &'a Path, cap: u64) -> Result<Self> {
        fs::create_dir_all(vault_dir.join(PACKS_DIR))?;
        let pack_id = existing_pack_ids(vault_dir)?.last().map_or(1, |id| id + 1);

        Ok(Self {
            vault_dir,
            cap,
            pack_id,
            offset: 0,
            file: None,
            extents: Vec::new(),
            start_pack_id: pack_id,
            start_offset: 0,
            named_a_pack: false,
            break_extent: false,
        })
    }

    /// The extents written so far, in order.
    #[must_use]
    pub fn extents(&self) -> &[Extent] {
        &self.extents
    }

    /// Ends the current extent, so the next write starts a new one.
    ///
    /// An ingest streams one entry through a sink and wants its writes merged
    /// into as few extents as possible. Reclaiming space streams *several*
    /// entries through one sink, and there the merge would be wrong: two
    /// entries' runs landing next to each other would become one extent
    /// belonging to whichever entry asked first. Sealing between them keeps
    /// each entry's copied run its own.
    pub fn seal_extent(&mut self) {
        self.break_extent = true;
    }

    /// Finishes the sink, fsyncing the pack — and, when this sink created one,
    /// the directory that names it — before anything may refer to it. This is
    /// the ordering FR-12 depends on.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the flush or fsync fails.
    pub fn finish(mut self) -> Result<Vec<Extent>> {
        if let Some(file) = self.file.take() {
            file.sync_all()?;
        }
        // Bytes durable under a name that is durable too (§4.7, HC-4).
        if self.named_a_pack {
            crate::durable::sync_dir(&self.vault_dir.join(PACKS_DIR))?;
        }
        Ok(self.extents)
    }

    /// Abandons everything this sink wrote, returning the packs to the state
    /// they were in when it opened, so a cancelled ingest leaves no bytes
    /// behind (FR-14).
    ///
    /// Safe because packs are append-only and the vault holds an exclusive
    /// lock: the only bytes above the starting offset are this sink's own.
    /// A *crash* still leaves orphans that no rollback ran for — that is what
    /// reconciliation is for (FR-32).
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if a pack cannot be truncated or removed.
    pub fn rollback(mut self) -> Result<()> {
        if let Some(file) = self.file.take() {
            drop(file);
        }

        // Every pack created after the starting one is entirely this sink's.
        for id in (self.start_pack_id + 1)..=self.pack_id {
            let path = pack_path(self.vault_dir, id);
            if path.exists() {
                fs::remove_file(path)?;
            }
        }

        let path = pack_path(self.vault_dir, self.start_pack_id);
        if path.exists() {
            if self.start_offset == 0 {
                fs::remove_file(path)?;
            } else {
                let file = fs::OpenOptions::new().write(true).open(path)?;
                file.set_len(self.start_offset)?;
                file.sync_all()?;
            }
        }
        // A removal is a change to the directory, so the directory is where it
        // has to become durable.
        crate::durable::sync_dir(&self.vault_dir.join(PACKS_DIR))?;
        Ok(())
    }

    fn ensure_open(&mut self) -> std::io::Result<()> {
        if self.file.is_some() {
            return Ok(());
        }
        let path = pack_path(self.vault_dir, self.pack_id);
        self.named_a_pack |= !path.exists();
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(false)
            .write(true)
            .read(false)
            .truncate(false)
            .open(path)?;
        file.seek(SeekFrom::Start(self.offset))?;
        self.file = Some(file);
        Ok(())
    }

    fn roll_over(&mut self) -> std::io::Result<()> {
        if let Some(file) = self.file.take() {
            // Durable before anything is written into its successor.
            file.sync_all()?;
        }
        self.pack_id += 1;
        self.offset = 0;
        Ok(())
    }

    fn record(&mut self, length: u64) {
        // Consecutive writes into one pack merge into one extent, unless a
        // caller has sealed the one before.
        let merge = !self.break_extent;
        self.break_extent = false;
        if let Some(last) = self.extents.last_mut()
            && merge
            && last.pack_id == self.pack_id
            && last.offset + last.length == self.offset
        {
            last.length += length;
        } else {
            self.extents.push(Extent {
                pack_id: self.pack_id,
                offset: self.offset,
                length,
            });
        }
        self.offset += length;
    }
}

impl Write for PackSink<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.offset >= self.cap {
            self.roll_over()?;
        }
        self.ensure_open()?;

        let room = self.cap.saturating_sub(self.offset);
        let take = buf.len().min(usize::try_from(room).unwrap_or(usize::MAX));

        let Some(file) = self.file.as_mut() else {
            return Err(std::io::Error::other("pack file not open"));
        };
        file.write_all(&buf[..take])?;
        self.record(take as u64);
        Ok(take)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self.file.as_mut() {
            Some(file) => file.flush(),
            None => Ok(()),
        }
    }
}

/// A reader over one entry's extents, in order. Seeks directly to each extent,
/// so one entry is readable without touching unrelated data (A-5).
pub struct PackSource<'a> {
    vault_dir: &'a Path,
    extents: Vec<Extent>,
    index: usize,
    consumed: u64,
    file: Option<fs::File>,
}

impl<'a> PackSource<'a> {
    /// Opens a reader over the given extents.
    #[must_use]
    pub fn new(vault_dir: &'a Path, extents: &[Extent]) -> Self {
        Self {
            vault_dir,
            extents: extents.to_vec(),
            index: 0,
            consumed: 0,
            file: None,
        }
    }
}

impl Read for PackSource<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            let Some(extent) = self.extents.get(self.index).copied() else {
                return Ok(0);
            };

            if self.consumed >= extent.length {
                self.index += 1;
                self.consumed = 0;
                self.file = None;
                continue;
            }

            if self.file.is_none() {
                let mut file = fs::File::open(pack_path(self.vault_dir, extent.pack_id))?;
                file.seek(SeekFrom::Start(extent.offset + self.consumed))?;
                self.file = Some(file);
            }

            let remaining = extent.length - self.consumed;
            let want = buf
                .len()
                .min(usize::try_from(remaining).unwrap_or(usize::MAX));

            let Some(file) = self.file.as_mut() else {
                return Err(std::io::Error::other("pack file not open"));
            };
            let read = file.read(&mut buf[..want])?;
            if read == 0 {
                // The extent claims more than the pack holds — a truncated
                // pack. Caught as an authentication failure above (HC-3).
                self.index += 1;
                self.consumed = 0;
                self.file = None;
                continue;
            }
            self.consumed += read as u64;
            return Ok(read);
        }
    }
}

/// A damage report naming one pack. Only the caller that knows which pack it
/// was reaching for can supply the id (S-4).
#[must_use]
pub fn damaged_pack(pack_id: u32, affected: Vec<EntryId>) -> Error {
    Error::Corrupt {
        what: crate::error::Damaged::Pack { id: pack_id },
        affected,
    }
}
