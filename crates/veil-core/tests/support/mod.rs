//! Test support: the logging guard (Phase 0 to-do P0.6).
//!
//! HC-1 forbids entry names, folder metadata, and content from reaching
//! `tracing` output — a log file that reconstructs the index defeats the
//! vault. §6 states the prohibition; this is the thing that notices when it is
//! violated.
//!
//! Capture is global and always on once `init` has run, so later phases get
//! the guard by running their operations rather than by remembering to ask for
//! it. The assertion is per scope: [`guarded`] fails a test when a marker
//! appears in anything logged inside it, and [`scan`] returns the violations
//! instead of asserting, which is what lets the canary prove the guard fires.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Mutex, MutexGuard, OnceLock};
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::prelude::*;

/// Everything logged since the process started, one entry per event.
fn captured() -> &'static Mutex<Vec<String>> {
    static CAPTURED: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    CAPTURED.get_or_init(|| Mutex::new(Vec::new()))
}

/// Serialises guarded scopes so that one test's events are not attributed to
/// another's. Integration tests in one binary run in parallel by default.
fn scope_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let m = LOCK.get_or_init(|| Mutex::new(()));
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Renders an event's message and every structured field into one string.
///
/// Fields are captured as well as the message: a name passed as
/// `tracing::info!(entry = %name, "opened")` never appears in the message and
/// would be invisible to a guard that only read messages.
struct Rendered(String);

impl Visit for Rendered {
    fn record_debug(&mut self, field: &Field, value: &dyn core::fmt::Debug) {
        use core::fmt::Write as _;
        let _ = write!(self.0, "{}={:?} ", field.name(), value);
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        use core::fmt::Write as _;
        let _ = write!(self.0, "{}={} ", field.name(), value);
    }
}

struct CaptureLayer;

impl<S: tracing::Subscriber> Layer<S> for CaptureLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut rendered = Rendered(format!("target={} ", event.metadata().target()));
        event.record(&mut rendered);
        captured()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(rendered.0);
    }
}

/// Installs the capture layer as the global subscriber. Idempotent.
pub fn init() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = tracing_subscriber::registry().with(CaptureLayer).try_init();
    });
}

/// Runs `f` and returns every event logged inside it that contains a marker.
///
/// Returns the violations rather than asserting, so that a test can prove the
/// guard detects a planted marker (T0.7). A guard nobody has watched fire is
/// indistinguishable from a guard that does nothing.
pub fn scan<R>(markers: &[&str], f: impl FnOnce() -> R) -> (R, Vec<String>) {
    init();
    let _scope = scope_lock();
    let start = captured()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .len();
    let result = f();
    let mut events = captured()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let violations = events[start..]
        .iter()
        .filter(|e| markers.iter().any(|m| e.contains(m)))
        .cloned()
        .collect();
    // Drained, so that a scope which has been checked cannot be counted twice.
    // This is what lets the canary plant a marker deliberately without leaving
    // it behind for `assert_all_captured_clean` to find, and it gives that
    // function its meaning: it covers exactly the events nobody guarded.
    events.truncate(start);
    (result, violations)
}

/// Runs `f` and fails the test if any marker reached a log line (HC-1).
pub fn guarded<R>(markers: &[&str], f: impl FnOnce() -> R) -> R {
    let (result, violations) = scan(markers, f);
    assert!(
        violations.is_empty(),
        "HC-1 violation: {} log event(s) disclosed a marker:\n{}",
        violations.len(),
        violations.join("\n")
    );
    result
}

/// Fails the test if any marker appears anywhere logged so far in this binary.
pub fn assert_all_captured_clean(markers: &[&str]) {
    init();
    // The same lock `scan` holds. Without it this reads the buffer while the
    // canary's deliberately planted marker is still in it — the whole suite in
    // one binary, tests in parallel — and the guard fails at random.
    let _scope = scope_lock();
    let events = captured()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let violations: Vec<_> = events
        .iter()
        .filter(|e| markers.iter().any(|m| e.contains(m)))
        .collect();
    assert!(
        violations.is_empty(),
        "HC-1 violation: {} log event(s) disclosed a marker",
        violations.len()
    );
}
