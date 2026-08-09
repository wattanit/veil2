//! Phase 0 test cases T0.9 and T0.10 — the logging guard (HC-1, Spec §6).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

/// A name of the shape HC-1 exists to keep out of a log file.
const MARKER: &str = "exec_compensation_2024.csv";

/// T0.9 — the guard fires.
///
/// Deliberately logs a marker as a message and, separately, as a structured
/// field. The guard must report a violation in both cases. If it stops
/// detecting either, this test fails — which is the point: the failure mode it
/// defends against is silent, and a guard that has never been seen to fire is
/// an assumption with a test name.
#[test]
fn t0_9_guard_detects_a_planted_marker() {
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

/// T0.9 — the guard does not fire on operational events.
///
/// §6 permits `tracing` for operational events: operation started, bytes
/// processed, error variant. A guard that also rejects those would be
/// abandoned within a week, which is its own kind of failure.
#[test]
fn t0_9_guard_permits_operational_events() {
    support::guarded(&[MARKER], || {
        tracing::info!(operation = "extract", bytes = 4_194_304_u64, "started");
        tracing::warn!(error = "Corrupt", "operation failed");
    });
}

/// T0.10 — the guard is on by default.
///
/// Capture is installed globally, so an event logged with no guard scope
/// around it is still recorded and still checked here.
///
/// *Standing limitation:* Phase 0 has no vault operations, so this currently
/// guards an empty surface. It is re-asserted against real operations in every
/// later phase, and it exists now so that the first operation is written under
/// it rather than the guard being retrofitted after the first leak.
#[test]
fn t0_10_no_marker_reaches_any_log_line() {
    support::init();
    tracing::info!(operation = "open", "vault opened");
    support::assert_all_captured_clean(&[MARKER]);
}

/// T0.11 — the guard holds across a real vault lifecycle (HC-1, §6).
///
/// T0.9 and T0.10 prove the mechanism: that it fires on a planted marker and
/// that it is on by default. Neither drives a real vault, which is the gap
/// this closes — add, extract, replace, verify, and delete are each exercised
/// here with a name, folder, and content chosen to be unmistakable if any of
/// it reached a log line.
#[test]
fn t0_11_the_guard_holds_across_a_real_vault_lifecycle() {
    use veil_core::crypto::{KdfParams, Password};
    use veil_core::vault::{Cancel, NoProgress, Vault};

    const MARKER_NAME: &str = "exec_compensation_2024.csv";
    const MARKER_FOLDER: &str = "HR/salaries";
    const MARKER_CONTENT: &str = "SALARY-ROW-MARKER-9c1f";

    let dir = std::env::temp_dir().join(format!("veil2-log-guard-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    support::guarded(&[MARKER_NAME, MARKER_FOLDER, MARKER_CONTENT], || {
        let password = Password::new("a sufficiently long password".to_owned());
        let mut vault = Vault::create(&dir, &password, KdfParams::for_tests()).unwrap();

        let mut content = MARKER_CONTENT.as_bytes().to_vec();
        content.extend(std::iter::repeat_n(0xAA_u8, 2000));
        let id = vault
            .add(
                MARKER_NAME,
                MARKER_FOLDER,
                &mut content.as_slice(),
                &mut NoProgress,
                &Cancel::new(),
            )
            .unwrap();

        let mut out = Vec::new();
        vault
            .extract(id, &mut out, &mut NoProgress, &Cancel::new())
            .unwrap();
        assert_eq!(out, content);

        let mut replacement = MARKER_CONTENT.as_bytes().to_vec();
        replacement.extend(std::iter::repeat_n(0xBB_u8, 2000));
        let id = vault
            .replace(
                MARKER_FOLDER,
                MARKER_NAME,
                &mut replacement.as_slice(),
                &mut NoProgress,
                &Cancel::new(),
            )
            .unwrap();

        vault.verify(&mut NoProgress, &Cancel::new()).unwrap();
        vault.delete(id).unwrap();
    });

    let _ = std::fs::remove_dir_all(&dir);
}
