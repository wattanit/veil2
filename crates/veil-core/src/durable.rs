//! Making a *name* durable, not only its contents (Spec §4.7; HC-4, FR-12).
//!
//! `fsync` on a file makes that file's bytes durable. It says nothing about the
//! directory entry that gives those bytes a name: that lives in the parent
//! directory and needs its own sync. Without it a crash can leave a perfectly
//! durable pack file that nothing can find, or a header renamed over on one
//! machine and not on another — a file whose contents survived and whose name
//! did not.
//!
//! §4.7 fixes the ordering between the pack and the index and is silent on the
//! directory, which is how this went unnoticed through three phases. Recorded
//! as *Notes for Upstream*, item 1 of the Phase 4 to-do list.
//!
//! **This is not the indirection layer that was rejected.** It calls the
//! operating system directly and there is nothing here to substitute, observe,
//! or inject. A test cannot use it to watch an ordering; only killing a process
//! can do that.

use std::path::Path;

use crate::error::Result;

/// Makes a directory's entries durable, so a file synced inside it cannot be
/// left nameless by a crash.
///
/// Call it *after* the file it names has been synced and *before* anything
/// claims the file exists.
///
/// # Errors
///
/// [`Error::Io`](crate::error::Error::Io) if the directory cannot be opened or
/// synced.
#[cfg(unix)]
pub fn sync_dir(dir: &Path) -> Result<()> {
    std::fs::File::open(dir)?.sync_all()?;
    Ok(())
}

/// Makes a directory's entries durable. No-op on this platform.
///
/// Windows offers no equivalent: a directory cannot be opened for syncing
/// without a raw handle flag, and NTFS makes directory updates durable through
/// its own metadata journal rather than on request. Development is on macOS;
/// this is documented behaviour rather than something measured here (Spec
/// §8.1).
///
/// # Errors
///
/// Never on this platform.
#[cfg(not(unix))]
pub fn sync_dir(_dir: &Path) -> Result<()> {
    Ok(())
}
