//! Changing what a vault already holds (Spec §4.5, §4.7; FR-13, FR-21).

use std::io::Read;

use crate::error::{Error, Result};
use crate::index::EntryId;

use super::{Cancel, Progress, Vault};

impl Vault {
    /// Replaces the entry at one full path with new content (FR-13).
    ///
    /// New content is written and made durable first, then **one** generation
    /// step both points the path at it and marks the old extents reclaimable.
    /// Remove-then-add would leave a window with zero intact versions (HC-4).
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`] if no file has that path; otherwise as
    /// [`add`](Self::add).
    pub fn replace(
        &mut self,
        folder: &str,
        name: &str,
        src: &mut impl Read,
        progress: &mut impl Progress,
        cancel: &Cancel,
    ) -> Result<EntryId> {
        self.begin_write()?;

        let Some(position) = self
            .document
            .entries
            .iter()
            .position(|e| e.folder == folder && e.name == name)
        else {
            return Err(Error::NotFound);
        };

        let id = EntryId::new(self.document.next_entry_id);
        let staged = self.stage(id, name, folder, src, progress, cancel)?;

        // Nothing below can fail before the single index write. That is what
        // keeps this one generation step rather than two.
        let old = self.document.entries.swap_remove(position);
        let old_stored: u64 = old.extents.iter().map(|x| x.length).sum();

        self.document.next_entry_id += 1;
        self.document.next_pack_id = self.document.next_pack_id.max(staged.next_pack_id);
        self.document.statistics.logical_bytes =
            self.document.statistics.logical_bytes - old.size + staged.entry.size;
        self.document.statistics.physical_bytes += staged.ciphertext_len;
        self.document.statistics.reclaimable_bytes += old_stored;
        self.document.entries.push(staged.entry);
        self.commit()?;
        Ok(id)
    }

    /// Removes an entry from the index (FR-21).
    ///
    /// The stored bytes stay until compaction and are counted into reclaimable
    /// bytes so the reported figures say so (FR-8, FR-29). They cannot be
    /// removed here: packs are shared and append-only, so removing them means
    /// rewriting the pack, which is compaction (FR-23).
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`] if no such entry exists; [`Error::ChangedOnDisk`]
    /// or [`Error::Io`].
    pub fn delete(&mut self, id: EntryId) -> Result<()> {
        self.begin_write()?;

        let Some(position) = self.document.entries.iter().position(|e| e.id == id) else {
            return Err(Error::NotFound);
        };

        let removed = self.document.entries.swap_remove(position);
        let stored: u64 = removed.extents.iter().map(|x| x.length).sum();

        self.document.statistics.entry_count -= 1;
        self.document.statistics.logical_bytes -= removed.size;
        self.document.statistics.reclaimable_bytes += stored;
        // `next_entry_id` is deliberately untouched: reissuing a deleted
        // entry's identifier would let its wrapped key decrypt under a live
        // entry's nonce.
        self.commit()
    }
}
