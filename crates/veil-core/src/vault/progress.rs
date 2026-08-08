//! Progress reporting and cooperative cancellation (Spec §2; A-3, FR-14,
//! FR-19).
//!
//! Both are parameters, never global state: the CLI passes no-ops, the GUI
//! passes a sink that marshals to its UI thread and a token its cancel button
//! sets.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// What a progress figure counts. Ingest and extraction count bytes; a folder
/// ingest and verification count entries (Spec §4.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// Plaintext bytes.
    Bytes,
    /// Whole entries.
    Entries,
}

/// One progress observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressReport {
    /// What `done` and `total` count.
    pub unit: Unit,
    /// How much is finished. Never decreases within one operation.
    pub done: u64,
    /// The whole, when it is known in advance. `None` for a pipe, a growing
    /// file, or any `Read` that is not a file — assuming a total there shows a
    /// bar that lies.
    pub total: Option<u64>,
}

/// Receives progress observations.
///
/// Implementations must not block: this is called at every chunk boundary, on
/// the thread doing the work.
pub trait Progress {
    /// Reports one observation.
    fn report(&mut self, report: ProgressReport);
}

/// A sink that discards everything, for callers that do not display progress.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoProgress;

impl Progress for NoProgress {
    fn report(&mut self, _report: ProgressReport) {}
}

impl<T: Progress + ?Sized> Progress for &mut T {
    fn report(&mut self, report: ProgressReport) {
        (**self).report(report);
    }
}

/// A cancellation token. Cloneable across threads, so a UI thread can cancel
/// work on a worker thread; checked at chunk boundaries.
///
/// One-way. A token that could be un-cancelled would let a race leave an
/// operation running that the user believes they stopped.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    /// A token that has not been cancelled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
