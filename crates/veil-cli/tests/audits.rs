//! Phase 3 test cases T3.29 and T3.30 — the two audits over everything the
//! command line says (Design §7; HC-1, HC-2, Spec §6).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use harness::{Scratch, run};

const COMMANDS: [&str; 9] = [
    "create",
    "add",
    "list",
    "save-copy",
    "replace",
    "delete",
    "check",
    "info",
    "password",
];

/// Everything the command line says: every help text, and both streams of a
/// run over the whole surface including its failure paths.
fn everything_it_says(scratch: &Scratch) -> String {
    let vault = scratch.vault_arg();
    let mut said = run(&["--help"]).everything();
    for command in COMMANDS {
        said.push_str(&run(&[command, "--help"]).everything());
    }

    let source = scratch.write("sources/report.pdf", "content");
    let destination = scratch.path("saved.pdf");
    let wrong = scratch.write("wrong.txt", "an entirely different password");

    let runs: Vec<Vec<String>> = vec![
        vec!["create".into(), vault.clone()],
        vec![
            "add".into(),
            vault.clone(),
            source.display().to_string(),
            "--folder".into(),
            "docs".into(),
        ],
        // Refused: the path is already held (FR-34).
        vec![
            "add".into(),
            vault.clone(),
            source.display().to_string(),
            "--folder".into(),
            "docs".into(),
        ],
        vec!["list".into(), vault.clone()],
        vec!["list".into(), vault.clone(), "--group".into()],
        vec!["info".into(), vault.clone()],
        vec!["check".into(), vault.clone()],
        vec![
            "save-copy".into(),
            vault.clone(),
            "docs/report.pdf".into(),
            "--to".into(),
            destination.display().to_string(),
        ],
        // Refused: the destination exists (FR-18).
        vec![
            "save-copy".into(),
            vault.clone(),
            "docs/report.pdf".into(),
            "--to".into(),
            destination.display().to_string(),
        ],
        // Refused: no such file.
        vec!["delete".into(), vault.clone(), "docs/ghost.pdf".into()],
        vec!["delete".into(), vault.clone(), "docs/report.pdf".into()],
    ];
    for args in runs {
        said.push_str(
            &scratch
                .veil(&args.iter().map(String::as_str).collect::<Vec<_>>())
                .everything(),
        );
    }

    // Refused: wrong password. Not through `veil()`, which supplies the right
    // one.
    said.push_str(&run(&["list", &vault, "--password-file", wrong.to_str().unwrap()]).everything());
    // Refused: no password at all.
    said.push_str(&run(&["list", &vault]).everything());

    said
}

/// T3.29 — the fixed vocabulary holds across the whole surface.
///
/// One word per thing, GUI and command line alike, is a product decision. The
/// command line is where it erodes first, because the implementation's own
/// vocabulary is right there in the source.
#[test]
fn t3_29_no_forbidden_word_reaches_the_surface() {
    const FORBIDDEN: [&str; 20] = [
        // The left column of Design §7's table.
        "container",
        "archive",
        "repository",
        "entry",
        "object",
        "blob",
        "directory",
        "passphrase",
        "master password",
        "import",
        "ingest",
        "export",
        "decrypt",
        "unlock",
        "compact",
        "vacuum",
        "garbage-collect",
        "validate",
        "integrity check",
        "scrub",
    ];
    const FORBIDDEN_CLAIMS: [&str; 6] = [
        "military-grade",
        "bank-level",
        "unbreakable",
        "100% secure",
        "hacker-proof",
        "your data is safe",
    ];

    let scratch = Scratch::new("vocabulary");
    let said = everything_it_says(&scratch).to_lowercase();

    // An audit that scanned nothing would pass. These two assertions are what
    // make the ones below mean something.
    assert!(
        said.len() > 2000,
        "the audit only collected {} characters, so it checked almost nothing",
        said.len()
    );
    for expected in ["vault", "folder", "save-copy", "copy", "password", "check"] {
        assert!(
            said.contains(expected),
            "the audit did not capture the surface: no \"{expected}\" in it"
        );
    }

    for word in FORBIDDEN {
        assert!(
            !said.contains(word),
            "the surface says \"{word}\", which Design §7 forbids"
        );
    }
    for claim in FORBIDDEN_CLAIMS {
        assert!(!said.contains(claim), "the surface claims \"{claim}\"");
    }
}

/// T3.30 — no output discloses what it must not.
///
/// Error text is where key material escapes, because the failure paths are the
/// ones nobody reads. The markers are shaped like the things HC-1 and HC-2
/// exist to keep out of sight.
#[test]
fn t3_30_nothing_leaks_into_any_output() {
    const CONTENT_MARKER: &str = "SALARY-ROW-MARKER-9c1f";
    const PASSWORD_MARKER: &str = "PASSWORD-MARKER-4d2e-long-enough";

    let scratch = Scratch::new("disclosure");
    // Replace the harness password with one that is recognisable in output.
    scratch.write("password.txt", PASSWORD_MARKER);

    let vault = scratch.vault_arg();
    assert_eq!(scratch.veil(&["create", &vault]).code, 0);

    let source = scratch.write("sources/notes.txt", CONTENT_MARKER);
    assert_eq!(
        scratch
            .veil(&["add", &vault, source.to_str().unwrap(), "--folder", "hr"])
            .code,
        0
    );

    let mut said = String::new();
    let destination = scratch.path("out.txt");
    for args in [
        vec!["list", &vault],
        vec!["info", &vault],
        vec!["check", &vault],
        vec![
            "save-copy",
            &vault,
            "hr/notes.txt",
            "--to",
            destination.to_str().unwrap(),
        ],
        vec!["delete", &vault, "hr/ghost.txt"],
        vec!["add", &vault, source.to_str().unwrap(), "--folder", "hr"],
        vec!["replace", &vault, "hr/ghost.txt", "--from", "/nonexistent"],
    ] {
        said.push_str(&scratch.veil(&args).everything());
    }

    // The failure paths, which is where this actually matters.
    said.push_str(&run(&["list", &vault]).everything());
    let wrong = scratch.write("wrong.txt", "an entirely different password");
    said.push_str(&run(&["list", &vault, "--password-file", wrong.to_str().unwrap()]).everything());

    assert!(
        !said.contains(PASSWORD_MARKER),
        "a password reached the output"
    );
    assert!(
        !said.contains(CONTENT_MARKER),
        "a file's content reached the output"
    );
}
