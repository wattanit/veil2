//! Advisory locking for an open vault (Spec §2; FR-26).
//!
//! An OS advisory lock on a lock file, held for the vault's lifetime.
//!
//! Advisory locks are unreliable on network filesystems and some FUSE mounts.
//! There the lock is best-effort and the index generation counter is the real
//! protection: the conflicting write is detected and refused, not prevented
//! (FR-27).

use std::fs;
use std::path::Path;

use fs4::{FileExt, TryLockError};

use crate::error::{Error, Result};

/// Name of the lock file, inside the vault so a copy of the directory is a copy
/// of the vault. A replicated copy is harmless: the lock lives in the OS, not
/// in the file's contents.
pub const LOCK_FILE: &str = "veil.lock";

/// Whether a vault may be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// The vault is locked and writable.
    ReadWrite,
    /// The vault opened without a lock because its storage would not take one.
    /// Read-only media must open (§4.5), and verification must run on them
    /// (§4.8).
    ReadOnly,
}

/// A held advisory lock, released when dropped.
#[derive(Debug)]
pub struct VaultLock {
    /// `None` when the vault opened read-only and holds no lock.
    file: Option<fs::File>,
    access: Access,
}

impl VaultLock {
    /// Takes the vault's lock, or reports the vault read-only if the storage
    /// will not take one.
    ///
    /// # Errors
    ///
    /// [`Error::VaultInUse`] when another opener holds it — a distinct
    /// condition from damage, because it sends the user somewhere else.
    pub fn acquire(vault_dir: &Path) -> Result<Self> {
        let path = vault_dir.join(LOCK_FILE);
        let file = match fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&path)
        {
            Ok(file) => file,
            // A lock file that cannot be created because the medium is
            // read-only is not a failure; it is a read-only vault.
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem
                ) =>
            {
                return Ok(Self {
                    file: None,
                    access: Access::ReadOnly,
                });
            }
            Err(e) => return Err(Error::from(e)),
        };

        match FileExt::try_lock(&file) {
            Ok(()) => Ok(Self {
                file: Some(file),
                access: Access::ReadWrite,
            }),
            // Contention is FR-26's condition and has its own error.
            Err(TryLockError::WouldBlock) => Err(Error::VaultInUse),
            // A lock the filesystem does not support is not contention.
            // "In use" would send the user hunting for a second window that
            // does not exist.
            Err(TryLockError::Error(_)) => Ok(Self {
                file: None,
                access: Access::ReadOnly,
            }),
        }
    }

    /// Whether the vault may be written.
    #[must_use]
    pub fn access(&self) -> Access {
        self.access
    }
}

impl Drop for VaultLock {
    fn drop(&mut self) {
        // `Drop` rather than a call at the end of a successful path, so the
        // release survives an error and an unwind. A leaked lock reports a
        // user's own vault as in use, with no obvious remedy.
        //
        // Closing the file releases the lock anyway; the explicit unlock states
        // it, and its failure is ignored because the close covers it.
        if let Some(file) = self.file.take() {
            let _ = FileExt::unlock(&file);
        }
    }
}
