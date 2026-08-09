//! Folder ingest: what gets stored, what gets skipped, and what gets said
//! about it (Spec §4.7; FR-10, FR-11).

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;

use super::normalize;

/// Why a path in an ingested folder was not stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// A symbolic link. Not followed: following risks cycles and captures data
    /// outside the tree the user selected (FR-11).
    SymbolicLink,
    /// Neither a regular file nor a directory — a socket, a device node, a
    /// FIFO. FR-10 stores regular files; storing the others would either block
    /// forever or record something that is not the user's data.
    NotARegularFile,
}

/// One path the walk declined to store. Returned rather than silently omitted:
/// a walk that drops links quietly produces a vault the user thinks is
/// complete (FR-11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// The path that was skipped, as encountered.
    pub path: PathBuf,
    /// Why.
    pub reason: SkipReason,
}

/// One regular file the walk found, with the folder metadata it will carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Absolute or caller-relative path to read from.
    pub path: PathBuf,
    /// Path relative to the added root, `/`-separated, excluding the file name
    /// (FR-7, FR-10). Empty for a file directly in the root.
    pub folder: String,
    /// The file's own name.
    pub name: String,
}

/// The result of walking one folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Walk {
    /// Every regular file, in the order encountered.
    pub files: Vec<Found>,
    /// Every path declined, with its reason.
    pub skipped: Vec<Skipped>,
}

/// Walks `root`, collecting regular files and recording what was skipped.
///
/// Links are detected with `symlink_metadata` at every level, directories
/// included — checking only files would let a directory link pull in a tree
/// from outside the root, or loop forever on a link to an ancestor.
///
/// The root itself is not tested; the caller named it.
///
/// # Errors
///
/// [`Error::Io`](crate::Error::Io) if a directory cannot be read.
pub fn walk(root: &Path) -> Result<Walk> {
    let mut out = Walk::default();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let mut children: Vec<PathBuf> = Vec::new();
        for entry in fs::read_dir(&dir)? {
            children.push(entry?.path());
        }
        // Directory order is undefined, and an ingest that stores files in a
        // different order on two machines makes a vault's bytes depend on where
        // it was written (HC-8).
        children.sort();

        for path in children {
            let meta = fs::symlink_metadata(&path)?;

            if meta.file_type().is_symlink() {
                out.skipped.push(Skipped {
                    path,
                    reason: SkipReason::SymbolicLink,
                });
                continue;
            }

            if meta.is_dir() {
                stack.push(path);
                continue;
            }

            if !meta.is_file() {
                out.skipped.push(Skipped {
                    path,
                    reason: SkipReason::NotARegularFile,
                });
                continue;
            }

            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                // Names are UTF-8 (§4.3). A lossy replacement would produce
                // an entry whose name does not match the file (HC-8).
                out.skipped.push(Skipped {
                    path: path.clone(),
                    reason: SkipReason::NotARegularFile,
                });
                continue;
            };
            // NFC now, not later: macOS's filesystem APIs hand back NFD, and
            // normalising here means everything downstream — including the
            // sort below — already sees the form the vault will store (§4.6).
            let name = normalize::nfc(name).into_owned();

            let Some(folder) = relative_folder(root, &path) else {
                out.skipped.push(Skipped {
                    path,
                    reason: SkipReason::NotARegularFile,
                });
                continue;
            };
            let folder = normalize::nfc(&folder).into_owned();

            out.files.push(Found { path, folder, name });
        }
    }

    out.files
        .sort_by(|a, b| (&a.folder, &a.name).cmp(&(&b.folder, &b.name)));
    out.skipped.sort();
    Ok(out)
}

/// The `/`-separated folder metadata for `path` beneath `root`. The separator
/// is never the host's, or a vault written on Windows would present different
/// folder strings on Linux (§4.6).
fn relative_folder(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let parent = relative.parent()?;
    let mut segments = Vec::new();
    for component in parent.components() {
        segments.push(component.as_os_str().to_str()?);
    }
    Some(segments.join("/"))
}

impl PartialOrd for Skipped {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Skipped {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.path.cmp(&other.path)
    }
}
