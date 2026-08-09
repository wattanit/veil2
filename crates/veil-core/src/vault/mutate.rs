//! Changing what a vault already holds (Spec §4.5, §4.7; FR-13, FR-22).

use std::io::Read;

use crate::error::{Error, Result};
use crate::index::EntryId;
use crate::store;

use super::{Cancel, Progress, Vault, normalize};

impl Vault {
    /// Replaces the entry at one full path with new content (FR-13).
    ///
    /// `folder` and `name` are normalised before the match, so either
    /// spelling of the same visible name finds the entry it identifies
    /// (§4.6). New content is written under a new id and made durable first,
    /// then **one** generation step both points the path at it and drops the
    /// old id; the old entry's file is removed afterward. Remove-then-add
    /// would leave a window with zero intact versions (HC-4).
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

        let normalized_folder = normalize::nfc(folder);
        let normalized_name = normalize::nfc(name);
        let Some(position) = self.document.entries.iter().position(|e| {
            e.folder.as_str() == normalized_folder.as_ref()
                && e.name.as_str() == normalized_name.as_ref()
        }) else {
            return Err(Error::NotFound);
        };

        let id = EntryId::new(self.document.next_entry_id);
        let entry = self.stage(id, name, folder, src, progress, cancel)?;

        // Nothing below can fail before the single index write. That is what
        // keeps this one generation step rather than two.
        let old = self.document.entries.swap_remove(position);

        self.document.next_entry_id += 1;
        self.document.entries.push(entry);
        self.commit()?;

        // The old file is removed only after the commit that drops it from
        // the index (Spec §4.5) — never before, or a crash between the two
        // would leave the index pointing at a file that is already gone.
        store::remove(&self.dir, old.id)?;
        Ok(id)
    }

    /// Removes an entry from the index and immediately frees its file
    /// (FR-22).
    ///
    /// The index removal is committed and fsynced first, then the file is
    /// removed — never the reverse (Spec §4.5). A crash between the two
    /// leaves a file the index no longer references, left alone as residue;
    /// it never leaves an index entry pointing at content that is already
    /// gone.
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
        // `next_entry_id` is deliberately untouched: reissuing a deleted
        // entry's identifier would let its wrapped key decrypt under a live
        // entry's nonce.
        self.commit()?;

        store::remove(&self.dir, removed.id)?;
        Ok(())
    }
}
