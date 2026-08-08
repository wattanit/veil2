//! Phase 3 test cases T3.16–T3.18 — which stream carries what, and what an
//! interrupt does (Design §3.4; A-3, FR-14, FR-19, HC-4).
//!
//! Both long-running cases feed the add through a named pipe. That makes the
//! timing the test's rather than the machine's: the child blocks until this
//! process writes, so "still running when the signal arrives" is arranged
//! instead of hoped for.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg(unix)]

mod harness;

use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use harness::Scratch;

static CHUNK: [u8; 1 << 20] = [7u8; 1 << 20];

/// Makes a named pipe and starts `veil add` reading from it. The child blocks
/// opening it until the returned writer is created.
fn add_through_a_pipe(scratch: &Scratch) -> (Child, std::fs::File) {
    let pipe = scratch.path("source.bin");
    let made = Command::new("mkfifo").arg(&pipe).status().unwrap();
    assert!(made.success(), "mkfifo failed");

    let child = Command::new(env!("CARGO_BIN_EXE_veil"))
        .args([
            "add",
            &scratch.vault_arg(),
            pipe.to_str().unwrap(),
            "--password-file",
            scratch.password_file().to_str().unwrap(),
        ])
        .env_remove("VEIL_PASSWORD")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    // Opening for writing blocks until the child opens for reading, so this
    // returning means the add is under way.
    let writer = std::fs::OpenOptions::new().write(true).open(&pipe).unwrap();
    (child, writer)
}

/// T3.16 and T3.17 — progress goes to standard error as plain periodic lines,
/// and results go to standard output.
///
/// A pipeline that has to strip progress out of its input is a pipeline that
/// will strip the wrong line one day. A log full of terminal control characters
/// is the other half of the same rule.
#[test]
fn t3_16_and_t3_17_progress_is_plain_lines_on_standard_error() {
    let scratch = Scratch::new("progress-streams");
    assert_eq!(scratch.veil(&["create", &scratch.vault_arg()]).code, 0);

    let (child, mut writer) = add_through_a_pipe(&scratch);

    // Long enough to cross the reporting interval more than once.
    for _ in 0..5 {
        writer.write_all(&CHUNK).unwrap();
        std::thread::sleep(Duration::from_millis(900));
    }
    drop(writer);

    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0));

    let out = String::from_utf8_lossy(&output.stdout);
    let err = String::from_utf8_lossy(&output.stderr);

    assert!(
        err.contains("adding"),
        "no progress reached standard error: {err:?}"
    );
    assert!(
        !out.contains("adding"),
        "progress polluted standard output: {out:?}"
    );
    assert!(
        out.contains("source.bin"),
        "the result did not reach standard output: {out:?}"
    );

    // Off a terminal: plain lines, no control characters at all.
    assert!(
        !err.contains('\r') && !err.contains('\x1b'),
        "control characters reached a redirected stream: {err:?}"
    );

    // And no more often than the interval. Four seconds of work cannot produce
    // a line per chunk.
    let lines = err.lines().filter(|l| l.contains("adding")).count();
    assert!(
        (1..=4).contains(&lines),
        "expected periodic lines, got {lines}:\n{err}"
    );
}

/// T3.18 — an interrupt cancels rather than kills.
///
/// The case that proves the command line can reach the cancellation Phase 2
/// built, rather than merely dying safely. HC-4 already makes a kill safe;
/// FR-14 asks for more than that — the vault as though it had not been started.
#[test]
fn t3_18_an_interrupt_cancels_and_leaves_nothing_behind() {
    let scratch = Scratch::new("interrupt");
    assert_eq!(scratch.veil(&["create", &scratch.vault_arg()]).code, 0);

    let (child, mut writer) = add_through_a_pipe(&scratch);
    let pid = child.id();

    writer.write_all(&CHUNK).unwrap();
    std::thread::sleep(Duration::from_millis(300));

    assert!(
        Command::new("kill")
            .args(["-INT", &pid.to_string()])
            .status()
            .unwrap()
            .success()
    );

    // More data, so the read the child is parked in returns and it reaches the
    // chunk boundary where cancellation is checked.
    for _ in 0..4 {
        if writer.write_all(&CHUNK).is_err() {
            break;
        }
    }
    drop(writer);

    let output = child.wait_with_output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(10),
        "an interrupt did not cancel: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("cancelled"),
        "the exit said nothing about what it left behind"
    );

    // FR-14: as though it had not been started.
    let listed = scratch.veil(&["list", &scratch.vault_arg()]);
    assert_eq!(listed.code, 0, "the vault did not survive the interrupt");
    assert!(
        listed.out.contains("No files"),
        "a cancelled add left a file behind: {}",
        listed.out
    );

    let stored: u64 = scratch
        .packs()
        .iter()
        .map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .sum();
    assert_eq!(
        stored, 0,
        "a cancelled add left {stored} bytes in the packs"
    );
}
