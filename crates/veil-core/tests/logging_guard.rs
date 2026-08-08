//! Phase 0 test cases T0.7 and T0.8 — the logging guard (HC-1, Spec §6).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

/// A name of the shape HC-1 exists to keep out of a log file.
const MARKER: &str = "exec_compensation_2024.csv";

/// T0.7 — the guard fires.
///
/// Deliberately logs a marker as a message and, separately, as a structured
/// field. The guard must report a violation in both cases. If it stops
/// detecting either, this test fails — which is the point: the failure mode it
/// defends against is silent, and a guard that has never been seen to fire is
/// an assumption with a test name.
#[test]
fn t0_7_guard_detects_a_planted_marker() {
    let (_, in_message) = support::scan(&[MARKER], || {
        tracing::info!("extracting {MARKER}");
    });
    assert_eq!(
        in_message.len(),
        1,
        "the guard did not detect a marker in an event's message"
    );

    let (_, in_field) = support::scan(&[MARKER], || {
        tracing::info!(entry = MARKER, "extracting");
    });
    assert_eq!(
        in_field.len(),
        1,
        "the guard did not detect a marker in a structured field; a name \
         passed as a field never appears in the message"
    );

    let (_, in_debug_field) = support::scan(&[MARKER], || {
        tracing::info!(entry = ?MARKER, "extracting");
    });
    assert_eq!(
        in_debug_field.len(),
        1,
        "the guard did not detect a marker recorded through Debug"
    );
}

/// T0.7 — the guard does not fire on operational events.
///
/// §6 permits `tracing` for operational events: operation started, bytes
/// processed, error variant. A guard that also rejects those would be
/// abandoned within a week, which is its own kind of failure.
#[test]
fn t0_7_guard_permits_operational_events() {
    support::guarded(&[MARKER], || {
        tracing::info!(operation = "extract", bytes = 4_194_304_u64, "started");
        tracing::warn!(error = "Corrupt", "operation failed");
    });
}

/// T0.8 — the guard is on by default.
///
/// Capture is installed globally, so an event logged with no guard scope
/// around it is still recorded and still checked here.
///
/// *Standing limitation:* Phase 0 has no vault operations, so this currently
/// guards an empty surface. It is re-asserted against real operations in every
/// later phase, and it exists now so that the first operation is written under
/// it rather than the guard being retrofitted after the first leak.
#[test]
fn t0_8_no_marker_reaches_any_log_line() {
    support::init();
    tracing::info!(operation = "open", "vault opened");
    support::assert_all_captured_clean(&[MARKER]);
}
