//! Progress reporting and cooperative cancellation (Spec §2; A-3, FR-14,
//! FR-19).
//!
//! **Both are parameters, never global state.** The CLI passes no-ops; the GUI
//! passes a sink that marshals to its UI thread and a token its cancel button
//! sets. A-3 exists because retrofitting either into a completed core is
//! expensive, which is why they are built before the operations that use them
//! rather than after.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// What a progress figure counts.
///
/// Ingest and extraction count bytes; verification counts entries, because the
/// Design Guideline's estimate is in time and entry counts are what a user can
/// hold in their head (Spec §4.8).
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
    /// The whole, where it is known before starting.
    ///
    /// `None` for a source whose length cannot be known in advance — a pipe, a
    /// growing file, any `Read` that is not a file. A caller that assumes a
    /// total shows a progress bar that lies.
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

/// A cancellation token.
///
/// Cloneable and shareable across threads, so a UI thread can cancel work
/// running on a worker thread. Cancellation is cooperative and checked at chunk
/// boundaries, which bounds its latency to the work of a chunk (Spec §2).
///
/// **Cancelling is one-way.** A token that could be un-cancelled would let a
/// race between the cancel and the reset leave an operation running that the
/// user believes they stopped.
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
