//! Pack files and extents (Spec §4.5; S-3, S-4, A-5, FR-25).
//!
//! Packs are append-only and capped. The cap is what satisfies three
//! requirements at once:
//!
//! - **S-3** — adding a file dirties one pack plus the index, so incremental
//!   backup and file-sync transfer bytes proportional to the change rather
//!   than to the vault.
//! - **S-4** — a damaged region costs only the entries with extents in that
//!   pack, and because extents map packs to entries, those entries are
//!   *enumerable*. Attribution is half the requirement: S-4 rejects one bad
//!   sector losing everything, and equally rejects one bad sector being
//!   indistinguishable from total loss.
//! - **FR-25** — compaction rewrites one pack at a time and therefore needs
//!   about one pack of working space regardless of vault size.
//!
//! The cap is a parameter rather than a constant. A multi-pack test that needs
//! gigabytes of fixture gets marked ignored within a month, and the
//! requirements it covers are among the most consequential in the format.

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::index::{Entry, EntryId, Extent};

/// Default maximum bytes in one pack file.
///
/// Initial; tunable. Smaller improves sync granularity (S-3) and damage
/// locality (S-4); larger reduces file count and per-pack overhead.
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

/// Total bytes held by every pack file on disk.
///
/// **A diagnostic and a test oracle, not a source for the statistics of
/// FR-22.** FR-22 requires the totals to be maintained incrementally and never
/// scanned; this function is the independent measurement those totals are
/// checked against, which is the only way an incremental counter's slow
/// divergence is ever caught.
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

/// Every entry with at least one extent in the given pack (S-4).
///
/// This is the attribution S-4 requires: a partial failure presented as a list
/// of unreadable files rather than as a failure of the vault.
#[must_use]
pub fn entries_in_pack(entries: &[Entry], pack_id: u32) -> Vec<EntryId> {
    entries
        .iter()
        .filter(|e| e.extents.iter().any(|x| x.pack_id == pack_id))
        .map(|e| e.id)
        .collect()
}

/// An append-only sink that rolls over to a new pack at the cap, recording
/// where everything landed.
///
/// Implements [`Write`], so content encryption streams straight into it and
/// neither layer needs to know about the other's chunking.
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
        })
    }

    /// The extents written so far, in order.
    #[must_use]
    pub fn extents(&self) -> &[Extent] {
        &self.extents
    }

    /// Finishes the sink, fsyncing the pack before anything may refer to it.
    ///
    /// **Ordering is what makes FR-12 true.** Pack data is durable before the
    /// index generation that references it advances, so a crash between the
    /// two leaves pack bytes that no index references — reclaimed later as
    /// garbage — and never an index entry pointing at bytes that were not
    /// durable.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the flush or fsync fails.
    pub fn finish(mut self) -> Result<Vec<Extent>> {
        if let Some(file) = self.file.take() {
            file.sync_all()?;
        }
        Ok(self.extents)
    }

    /// Abandons everything this sink wrote, returning the packs to the state
    /// they were in when it opened.
    ///
    /// **Why truncate rather than leave the bytes as garbage.** §4.7 requires a
    /// cancelled ingest to leave a vault indistinguishable from one where the
    /// operation never started, and §4.5's reconciliation (FR-32, Phase 4)
    /// reclaims orphans only at the next open. Between those two facts sits a
    /// vault whose statistics disagree with its packs. Undoing the write here
    /// is safe because packs are append-only and the vault's exclusive lock
    /// means nobody else appended in the meantime — so the only bytes above the
    /// starting offset are this sink's own.
    ///
    /// Reconciliation is still needed: a *crash* leaves orphans that no
    /// rollback ran for. This narrows the window rather than closing it.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if a pack cannot be truncated or removed. The caller is
    /// already on a failure path; a failure here means the packs hold bytes the
    /// index does not reference, which Phase 4's reconciliation removes.
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
        Ok(())
    }

    fn ensure_open(&mut self) -> std::io::Result<()> {
        if self.file.is_some() {
            return Ok(());
        }
        let path = pack_path(self.vault_dir, self.pack_id);
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
            // The pack being left behind must be durable before anything is
            // written into its successor.
            file.sync_all()?;
        }
        self.pack_id += 1;
        self.offset = 0;
        Ok(())
    }

    fn record(&mut self, length: u64) {
        // Consecutive writes into one pack are one extent, not many.
        if let Some(last) = self.extents.last_mut()
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

/// A reader over one entry's extents, in order.
///
/// Reads seek directly to each extent, so one entry is readable without
/// touching unrelated data (A-5) — the door held open for the mount deferral,
/// and the basis of the product's first motivation.
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
                // The extent claims more than the pack holds: the pack was
                // truncated. Reported as short here, and caught as an
                // authentication failure by the layer above (HC-3).
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

/// Converts an I/O failure while touching a pack into a damage report for it.
///
/// The pack id is the attribution S-4 needs, and only the caller that knows
/// which pack it was reaching for can supply it.
#[must_use]
pub fn damaged_pack(pack_id: u32, affected: Vec<EntryId>) -> Error {
    Error::Corrupt {
        what: crate::error::Damaged::Pack { id: pack_id },
        affected,
    }
}
