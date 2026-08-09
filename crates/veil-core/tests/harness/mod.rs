//! Shared fixtures for the Phase 2 suite.
//!
//! Every case here drives the public API (A-1, A-4). Where a case must observe
//! something the API does not return — a file that appeared, a byte that
//! changed — it observes the filesystem, never a crate-private path.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use veil_core::Error;
use veil_core::crypto::{KdfParams, Password};
use veil_core::index::{EntryId, Statistics};
use veil_core::vault::{Cancel, NoProgress, Progress, ProgressReport, Vault};

pub fn password() -> Password {
    Password::new("a sufficiently long password".to_owned())
}

pub fn other_password(which: &str) -> Password {
    Password::new(format!("a different sufficiently long password {which}"))
}

/// A temporary directory removed when the test ends, pass or fail.
pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("veil2-p2-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub fn vault_dir(&self) -> PathBuf {
        self.0.join("Test.veil")
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Deterministic content: any difference after a round trip is a real one.
pub fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

pub fn create(dir: &Path) -> Vault {
    Vault::create(dir, &password(), KdfParams::for_tests()).unwrap()
}

pub fn open(dir: &Path) -> Result<Vault, Error> {
    Vault::open(dir, &password())
}

pub fn add(vault: &mut Vault, name: &str, folder: &str, content: &[u8]) -> EntryId {
    vault
        .add(
            name,
            folder,
            &mut &content[..],
            &mut NoProgress,
            &Cancel::new(),
        )
        .unwrap()
}

pub fn read_back(vault: &Vault, id: EntryId) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    vault.extract(id, &mut out, &mut NoProgress, &Cancel::new())?;
    Ok(out)
}

/// Every file under `root`, by relative path, with its full contents.
///
/// Used to assert that something did *not* change. Comparing contents rather
/// than modification times is deliberate: a filesystem's timestamp granularity
/// is coarse enough that a rewrite within the same second looks like no write.
pub fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(bytes) = std::fs::read(&path) {
                let relative = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(relative, bytes);
            }
        }
    }
    out
}

/// Records every progress observation, so a case can assert the shape of the
/// sequence rather than merely that something was reported.
#[derive(Debug, Default)]
pub struct Recorder(pub Vec<ProgressReport>);

impl Progress for Recorder {
    fn report(&mut self, report: ProgressReport) {
        self.0.push(report);
    }
}

/// Asserts the observations are non-empty and never go backwards.
///
/// A sink called once at the end satisfies "reports progress" and is useless to
/// a progress bar, which is why monotonic growth is asserted rather than mere
/// presence.
pub fn assert_monotonic(reports: &[ProgressReport], label: &str) {
    assert!(!reports.is_empty(), "{label}: nothing was reported");
    let mut last = 0;
    for report in reports {
        assert!(
            report.done >= last,
            "{label}: progress went backwards, {last} then {}",
            report.done
        );
        last = report.done;
    }
}

/// A progress sink that cancels once a threshold is passed.
///
/// Cancelling from the sink is how a UI cancel button reaches a running
/// operation: the token is shared, and the operation notices at its next chunk
/// boundary.
pub struct CancelAt {
    pub cancel: Cancel,
    pub after: u64,
    pub seen: Vec<ProgressReport>,
}

impl CancelAt {
    pub fn new(cancel: Cancel, after: u64) -> Self {
        Self {
            cancel,
            after,
            seen: Vec::new(),
        }
    }
}

impl Progress for CancelAt {
    fn report(&mut self, report: ProgressReport) {
        self.seen.push(report);
        if report.done >= self.after {
            self.cancel.cancel();
        }
    }
}

/// A source that yields `len` bytes and counts how many were taken.
///
/// The count is what makes cancellation latency observable: a token that stops
/// the work only after the whole source has been read is a button that does
/// nothing on the only files large enough for anyone to press it.
pub struct CountingSource {
    remaining: usize,
    pub taken: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    /// Called after every read, so a test can cancel at a chosen offset.
    pub on_read: Option<Box<dyn FnMut(usize)>>,
}

impl CountingSource {
    pub fn new(len: usize) -> Self {
        Self {
            remaining: len,
            taken: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            on_read: None,
        }
    }
}

impl std::io::Read for CountingSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = buf.len().min(self.remaining);
        for (offset, slot) in buf[..n].iter_mut().enumerate() {
            *slot = (offset % 251) as u8;
        }
        self.remaining -= n;
        let total = self.taken.fetch_add(n, std::sync::atomic::Ordering::SeqCst) + n;
        if let Some(hook) = self.on_read.as_mut() {
            hook(total);
        }
        Ok(n)
    }
}

/// Flips one byte at the given offset of an entry's own file.
pub fn flip_byte_in_entry_file(vault_dir: &Path, id: EntryId, at: u64) {
    let path = veil_core::store::entry_path(vault_dir, id);
    let mut bytes = std::fs::read(&path).unwrap();
    let index = usize::try_from(at).unwrap().min(bytes.len() - 1);
    bytes[index] ^= 0xff;
    std::fs::write(&path, bytes).unwrap();
}

/// Statistics are derived on every call (FR-7); this asserts an independent
/// sum over the resident entries agrees with what `statistics()` returned,
/// which is what would catch a divergence if one were ever introduced.
pub fn assert_statistics_correct(vault: &Vault, label: &str) {
    let held: Statistics = vault.statistics();
    let counted = Statistics::from_entries(vault.entries());
    assert_eq!(
        held, counted,
        "{label}: statistics diverged from a direct sum"
    );
}
