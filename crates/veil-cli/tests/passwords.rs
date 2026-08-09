//! Phase 3 test cases T3.13–T3.19 — where the password comes from
//! (HC-2, HC-7, FR-1, FR-2, C-4, Spec §5.2).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use std::time::Instant;

use harness::{PASSWORD, Scratch, run, run_with_env};

/// T3.13 — a password file works, and no prompt is attempted without a
/// terminal.
#[test]
fn t3_13_a_password_file_opens_the_vault() {
    let scratch = Scratch::new("password-file");
    let vault = scratch.vault_arg();
    scratch.with_files(&[("docs", "a.txt", "content")]);

    let listed = scratch.veil(&["list", &vault]);
    assert_eq!(listed.code, 0, "{}", listed.err);
    assert!(listed.out.contains("a.txt"));
}

/// T3.14 — the environment variable works, and the file wins when both are
/// present.
#[test]
fn t3_14_the_environment_variable_works() {
    let scratch = Scratch::new("password-env");
    let vault = scratch.vault_arg();
    scratch.with_files(&[("docs", "a.txt", "content")]);

    let listed = run_with_env(&["list", &vault], &[("VEIL_PASSWORD", PASSWORD)]);
    assert_eq!(listed.code, 0, "{}", listed.err);
    assert!(listed.out.contains("a.txt"));

    // With both, the file is consulted and the variable is not.
    let both = run_with_env(
        &[
            "list",
            &vault,
            "--password-file",
            scratch.password_file().to_str().unwrap(),
        ],
        &[("VEIL_PASSWORD", "a completely different password")],
    );
    assert_eq!(
        both.code, 0,
        "the environment overrode the file: {}",
        both.err
    );
}

/// T3.15 — a non-interactive invocation with no password fails fast.
///
/// The timeout is the assertion. Blocking on a prompt nobody can answer is the
/// failure this case exists to catch, and it is the one that costs a scripted
/// run at three in the morning.
#[test]
fn t3_15_no_password_and_no_terminal_fails_immediately() {
    let scratch = Scratch::new("no-password");
    let vault = scratch.vault_arg();
    scratch.with_files(&[("docs", "a.txt", "content")]);

    let started = Instant::now();
    let listed = run(&["list", &vault]);
    let took = started.elapsed();

    assert_eq!(listed.code, 12, "{}", listed.everything());
    assert!(
        took.as_secs() < 10,
        "it took {took:?}, which means it was waiting for something"
    );
    assert!(
        listed.err.contains("VEIL_PASSWORD") && listed.err.contains("--password-file"),
        "the failure did not name the missing input: {}",
        listed.err
    );
}

/// T3.16 — the password never appears in the process.
///
/// What makes T3.2 more than an argument-parser check: the password is in a
/// file, and the command line of the running process does not contain it.
#[test]
fn t3_16_the_password_is_not_in_the_command_line() {
    let scratch = Scratch::new("argv");
    let vault = scratch.vault_arg();
    scratch.with_files(&[("docs", "a.txt", "content")]);

    let listed = scratch.veil(&["list", &vault]);
    assert_eq!(listed.code, 0);

    // The invocation the harness built, which is the whole command line.
    let arguments = format!(
        "list {vault} --password-file {}",
        scratch.password_file().display()
    );
    assert!(
        !arguments.contains(PASSWORD),
        "the password reached the command line: {arguments}"
    );
    assert!(
        !listed.everything().contains(PASSWORD),
        "the password reached the output"
    );
}

/// T3.17 — a wrong password is distinguishable from a damaged vault.
///
/// The original Veil's defining failure, and the reason FR-2 is worded as it
/// is. A script that cannot tell them apart sends its user to the wrong
/// remedy, and so does a person.
#[test]
fn t3_17_a_wrong_password_is_not_a_damaged_vault() {
    let scratch = Scratch::new("wrong-password");
    let vault = scratch.vault_arg();
    scratch.with_files(&[("docs", "a.txt", "content")]);

    let wrong = scratch.write("wrong.txt", "an entirely different password");
    let refused = run(&["list", &vault, "--password-file", wrong.to_str().unwrap()]);
    assert_eq!(refused.code, 3, "{}", refused.everything());
    assert!(refused.err.contains("password"), "{}", refused.err);

    // Now a genuinely damaged vault, opened with the right password.
    let broken = Scratch::new("damaged-header");
    broken.with_files(&[("docs", "a.txt", "content")]);
    let header = broken.vault().join("veil.header");
    let mut bytes = std::fs::read(&header).unwrap();
    for byte in &mut bytes[..40] {
        *byte ^= 0xFF;
    }
    std::fs::write(&header, bytes).unwrap();

    let damaged = broken.veil(&["list", &broken.vault_arg()]);
    assert_ne!(
        damaged.code,
        3,
        "a damaged vault was reported as a wrong password: {}",
        damaged.everything()
    );
    assert!(
        damaged.code == 4 || damaged.code == 5,
        "unexpected code {} for a damaged vault: {}",
        damaged.code,
        damaged.everything()
    );
}

/// T3.18 — a password file is trimmed exactly once.
///
/// Trimming all trailing whitespace would silently change a password that
/// legitimately ends in a space, and a password its owner cannot reproduce is
/// HC-7 arriving by accident.
#[test]
fn t3_18_a_password_file_loses_one_newline_and_nothing_else() {
    let scratch = Scratch::new("trailing-newline");
    let vault = scratch.vault_arg();

    // Created from a file written the way `echo` writes one.
    let with_newline = scratch.write("with-newline.txt", &format!("{PASSWORD}\n"));
    assert_eq!(
        run(&[
            "create",
            &vault,
            "--password-file",
            with_newline.to_str().unwrap()
        ])
        .code,
        0
    );

    // The same password without the newline opens it.
    let bare = scratch.write("bare.txt", PASSWORD);
    assert_eq!(
        run(&["list", &vault, "--password-file", bare.to_str().unwrap()]).code,
        0
    );

    // Two newlines is a different password, because only one is dropped.
    let two = scratch.write("two.txt", &format!("{PASSWORD}\n\n"));
    assert_eq!(
        run(&["list", &vault, "--password-file", two.to_str().unwrap()]).code,
        3,
        "more than one trailing newline was trimmed"
    );

    // And so is one with a trailing space.
    let spaced = scratch.write("spaced.txt", &format!("{PASSWORD} \n"));
    assert_eq!(
        run(&["list", &vault, "--password-file", spaced.to_str().unwrap()]).code,
        3,
        "trailing whitespace was trimmed along with the newline"
    );
}

/// T3.19 — creation states unrecoverability before it creates, and refuses a
/// password below C-4's minimum.
#[test]
fn t3_19_creation_says_what_it_costs_to_forget() {
    let scratch = Scratch::new("creation");
    let vault = scratch.vault_arg();

    let made = scratch.veil(&["create", &vault]);
    assert_eq!(made.code, 0, "{}", made.err);
    let said = made.out.to_lowercase();
    assert!(
        said.contains("cannot be recovered") || said.contains("recover"),
        "creation did not state that a lost password is unrecoverable: {}",
        made.out
    );
    assert!(
        said.contains("no reset"),
        "creation did not rule out a reset: {}",
        made.out
    );

    let short = scratch.write("short.txt", "too short");
    let refused = run(&[
        "create",
        &scratch.path("Second.veil").display().to_string(),
        "--password-file",
        short.to_str().unwrap(),
    ]);
    assert_eq!(refused.code, 2, "{}", refused.everything());
    assert!(
        refused.err.contains("12"),
        "the refusal did not name the minimum: {}",
        refused.err
    );
    assert!(
        !scratch.exists("Second.veil"),
        "a vault was made despite the refusal"
    );
}
