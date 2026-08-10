//! Phase 3 test cases T3.1–T3.6 — the command surface
//! (A-4, FR-2, FR-10, FR-11, FR-13, FR-14, FR-23, HC-2).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use harness::{Scratch, run};

/// T3.1 — every core capability is reachable from the shell.
///
/// A capability that needs the library to reach it is a parity defect (A-4),
/// which is this phase's central requirement rather than a nicety.
#[test]
fn t3_1_the_whole_lifecycle_runs_from_the_shell() {
    let scratch = Scratch::new("lifecycle");
    let vault = scratch.vault_arg();

    assert_eq!(scratch.veil(&["create", &vault]).code, 0);

    let source = scratch.write("sources/report.pdf", "the original content");
    scratch.write("tree/photos/one.jpg", "first");
    scratch.write("tree/photos/two.jpg", "second");

    let add = scratch.veil(&["add", &vault, source.to_str().unwrap(), "--folder", "work"]);
    assert_eq!(add.code, 0, "{}", add.err);

    let add_folder = scratch.veil(&["add", &vault, scratch.path("tree").to_str().unwrap()]);
    assert_eq!(add_folder.code, 0, "{}", add_folder.err);

    let list = scratch.veil(&["list", &vault]);
    assert_eq!(list.code, 0);
    assert!(list.out.contains("report.pdf"), "{}", list.out);
    assert!(list.out.contains("one.jpg"), "{}", list.out);

    let saved = scratch.path("saved.pdf");
    let save = scratch.veil(&[
        "save-copy",
        &vault,
        "work/report.pdf",
        "--to",
        saved.to_str().unwrap(),
    ]);
    assert_eq!(save.code, 0, "{}", save.err);
    assert_eq!(
        std::fs::read_to_string(&saved).unwrap(),
        "the original content"
    );

    let replacement = scratch.write("sources/newer.pdf", "content that replaced it");
    let replace = scratch.veil(&[
        "replace",
        &vault,
        "work/report.pdf",
        "--from",
        replacement.to_str().unwrap(),
    ]);
    assert_eq!(replace.code, 0, "{}", replace.err);

    assert_eq!(scratch.veil(&["check", &vault]).code, 0);
    assert_eq!(scratch.veil(&["info", &vault]).code, 0);

    let new_password = scratch.write("new-password.txt", "an entirely new password");
    let change = scratch.veil(&[
        "password",
        &vault,
        "--new-password-file",
        new_password.to_str().unwrap(),
    ]);
    assert_eq!(change.code, 0, "{}", change.err);

    let reopened = run(&[
        "list",
        &vault,
        "--password-file",
        new_password.to_str().unwrap(),
    ]);
    assert_eq!(reopened.code, 0, "{}", reopened.err);

    let delete = run(&[
        "delete",
        &vault,
        "tree/photos/one.jpg",
        "--password-file",
        new_password.to_str().unwrap(),
    ]);
    assert_eq!(delete.code, 0, "{}", delete.err);

    let after = run(&[
        "check",
        &vault,
        "--password-file",
        new_password.to_str().unwrap(),
    ]);
    assert_eq!(after.code, 0, "the vault did not survive its own lifecycle");
}

/// T3.2 — no command accepts a password as an argument.
///
/// Arguments appear in process listings and shell history. An option that
/// exists and is merely discouraged is one `history | grep` away from being
/// the disclosure.
#[test]
fn t3_2_no_command_takes_a_password_as_an_argument() {
    let scratch = Scratch::new("no-password-argument");
    let vault = scratch.vault_arg();

    for command in [
        "create",
        "add",
        "list",
        "save-copy",
        "replace",
        "delete",
        "check",
        "info",
        "password",
    ] {
        let run = run(&[command, &vault, "--password", "secret"]);
        assert_eq!(
            run.code,
            2,
            "`{command}` accepted --password: {}",
            run.everything()
        );
        assert!(
            run.err.contains("--password") && run.err.contains("unexpected"),
            "`{command}` failed for the wrong reason: {}",
            run.err
        );
    }
}

/// T3.3 — nothing schedules or conditions an operation.
///
/// FR-23 forbids automatic compaction, and a switch a user can wire into cron
/// is that prohibition defeated under another name.
#[test]
fn t3_3_nothing_can_be_scheduled() {
    const FORBIDDEN: [&str; 7] = [
        "--schedule",
        "--daemon",
        "--watch",
        "--interval",
        "--cron",
        "--every",
        "--threshold",
    ];

    let mut help = run(&["--help"]).everything();
    for command in [
        "create",
        "add",
        "list",
        "save-copy",
        "replace",
        "delete",
        "check",
        "info",
        "password",
    ] {
        help.push_str(&run(&[command, "--help"]).everything());
    }

    for flag in FORBIDDEN {
        assert!(!help.contains(flag), "the surface carries {flag}");
    }
}

/// T3.4 — a path the vault already holds is refused (FR-14).
#[test]
fn t3_4_a_path_already_held_is_refused() {
    let scratch = Scratch::new("already-held");
    let vault = scratch.vault_arg();
    scratch.with_files(&[("docs", "report.pdf", "the first content")]);

    let second = scratch.write("other/report.pdf", "different content entirely");
    let run = scratch.veil(&["add", &vault, second.to_str().unwrap(), "--folder", "docs"]);

    assert_eq!(run.code, 13, "{}", run.everything());
    assert!(run.err.contains("already holds"), "{}", run.err);
    assert!(
        run.err.contains("replace"),
        "the refusal did not say what to do instead"
    );

    // The refusal left the file it refused to replace alone.
    let saved = scratch.path("check.pdf");
    assert_eq!(
        scratch
            .veil(&[
                "save-copy",
                &vault,
                "docs/report.pdf",
                "--to",
                saved.to_str().unwrap()
            ])
            .code,
        0
    );
    assert_eq!(
        std::fs::read_to_string(&saved).unwrap(),
        "the first content",
        "the refused add damaged the file it refused to replace"
    );
}

/// T3.5 — a path matching nothing is not reported as damage.
///
/// A typed path is the commonest mistake there is. Sending it to "your vault
/// may be corrupted" is the FR-2 failure one level down.
#[test]
fn t3_5_a_path_matching_nothing_is_not_damage() {
    let scratch = Scratch::new("no-such-file");
    let vault = scratch.vault_arg();
    scratch.with_files(&[("docs", "real.txt", "content")]);

    for args in [
        vec!["delete", &vault, "docs/ghost.txt"],
        vec!["save-copy", &vault, "docs/ghost.txt", "--to", "/dev/null"],
    ] {
        let run = scratch.veil(&args);
        assert_eq!(run.code, 13, "{}", run.everything());
        assert!(
            run.err.contains("docs/ghost.txt"),
            "the message did not name the path: {}",
            run.err
        );
        assert!(
            !run.err.contains("damage") && !run.err.contains("damaged"),
            "a mistyped name was reported as damage: {}",
            run.err
        );
    }
}

/// T3.6 — a folder add reports what it skipped.
///
/// A link silently omitted is a file the user believes is in the vault.
#[test]
#[cfg(unix)]
fn t3_6_a_folder_add_names_what_it_skipped() {
    let scratch = Scratch::new("folder-skips");
    let vault = scratch.vault_arg();
    assert_eq!(scratch.veil(&["create", &vault]).code, 0);

    scratch.write("tree/kept.txt", "stored");
    scratch.write("tree/nested/also-kept.txt", "stored too");
    let outside = scratch.write("outside.txt", "not part of the tree");
    std::os::unix::fs::symlink(&outside, scratch.path("tree/a-link.txt")).unwrap();

    let run = scratch.veil(&["add", &vault, scratch.path("tree").to_str().unwrap()]);
    assert_eq!(run.code, 0, "{}", run.everything());
    assert!(run.out.contains("kept.txt"));
    assert!(run.out.contains("also-kept.txt"));
    assert!(
        run.out.contains("a-link.txt") && run.out.contains("link"),
        "the skipped link was not named: {}",
        run.out
    );

    let listed = scratch.veil(&["list", &vault]);
    assert!(
        !listed.out.contains("a-link.txt"),
        "a link was followed and stored"
    );
    assert!(listed.out.contains("nested"), "the folder was not recorded");
}
