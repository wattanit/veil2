//! Advisory locking for an open vault (Spec §2; FR-26).
//!
//! An OS advisory lock on a lock file, held for the lifetime of the open vault.
//!
//! *Honesty clause, restated from Spec §2 because it is the kind of thing that
//! gets lost between a document and a call site:* advisory locks are unreliable
//! on network filesystems and some FUSE-backed mounts. There the lock is
//! best-effort, and the index generation counter is the actual protection —
//! Veil2 detects the conflicting write and refuses rather than preventing it
//! (FR-27). The network-path advisory the product shows is P5.4.

use std::fs;
use std::path::Path;

use fs4::{FileExt, TryLockError};

use crate::error::{Error, Result};

/// Name of the lock file within a vault directory.
///
/// Inside the vault, so the vault stays self-contained and a copy of the
/// directory is a copy of the vault. The cost is that a sync tool replicates
/// this file; it holds no data, and a stale copy is harmless because the lock
/// lives in the OS, not in the file's contents.
pub const LOCK_FILE: &str = "veil.lock";

/// Whether a vault may be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// The vault is locked and writable.
    ReadWrite,
    /// The vault opened without a lock because its storage would not take one.
    ///
    /// §4.5 requires read-only media to open, and §4.8 requires verification to
    /// run on a read-only vault. Refusing to open would make the operation that
    /// diagnoses a failing drive the one operation a failing drive cannot run.
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
    /// [`Error::VaultInUse`] when another opener holds the lock. That is a
    /// distinct condition from damage and from an I/O failure, because it sends
    /// the user somewhere else entirely (FR-26, FR-2).
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
            // The distinction that matters: a lock file that cannot be created
            // because the medium is read-only is not a failure, it is a vault
            // that opens read-only. Any other I/O failure is a failure.
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
            // Contention is FR-26's condition and has its own error, because it
            // sends the user somewhere no other failure does.
            Err(TryLockError::WouldBlock) => Err(Error::VaultInUse),
            // A lock the filesystem does not support is not contention. Saying
            // "in use" here would send a user hunting for a second window that
            // does not exist; the vault opens read-only and FR-27's generation
            // counter is the protection, exactly as §2's honesty clause says.
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
        // **The release must survive an error path and an unwind**, which is
        // the whole reason this is a `Drop` rather than a call at the end of a
        // successful operation. A leaked lock reports a user's own vault as in
        // use, and the remedy is a file they were never told about.
        //
        // Closing the file releases the lock on every platform Veil2 supports;
        // the explicit unlock is here so the release is stated rather than
        // inferred, and its failure is ignored because there is no caller left
        // to tell and the close covers it.
        if let Some(file) = self.file.take() {
            let _ = FileExt::unlock(&file);
        }
    }
}
