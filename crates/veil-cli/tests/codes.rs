//! Phase 3 test cases T3.19–T3.28 — one exit code per condition, and the
//! refusals that carry them (Spec §5.2, §6; FR-2, FR-15, FR-17, FR-18, FR-21,
//! FR-26, FR-29, FR-33, S-4).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use harness::{PASSWORD, Scratch, run};

/// T3.19 — every condition this phase can provoke has its own code, and the
/// codes are documented where a script author will find them.
///
/// The conditions **not** provoked here are named rather than quietly omitted.
/// Changed-on-disk (7) cannot be reached from a command line whose every
/// invocation opens and commits within one process. Storage-unavailable (11)
/// needs a disk to be pulled out mid-write. Cancelled (10) is T3.18's. The
/// mapping for all three is checked by the compiler instead: the match in
/// `Failure::code` is exhaustive over an error type that is deliberately not
/// `#[non_exhaustive]`, so a variant without a code does not build.
#[test]
fn t3_19_each_condition_has_its_own_code() {
    let scratch = Scratch::new("codes");
    let vault = scratch.vault_arg();
    scratch.with_files(&[("docs", "a.txt", "content")]);

    let mut seen: Vec<(i32, &str)> = Vec::new();

    seen.push((scratch.veil(&["list", &vault]).code, "success"));
    seen.push((run(&["list", &vault, "--nonsense"]).code, "usage"));

    let wrong = scratch.write("wrong.txt", "a different long password");
    seen.push((
        run(&["list", &vault, "--password-file", wrong.to_str().unwrap()]).code,
        "wrong password",
    ));
    seen.push((
        scratch
            .veil(&["list", &scratch.path("nowhere.veil").display().to_string()])
            .code,
        "not a vault",
    ));
    seen.push((run(&["list", &vault]).code, "no password to be had"));
    seen.push((
        scratch.veil(&["delete", &vault, "docs/ghost.txt"]).code,
        "no such file",
    ));

    for (code, condition) in &seen {
        let clashes = seen
            .iter()
            .filter(|(other, name)| other == code && name != condition)
            .count();
        assert_eq!(
            clashes, 0,
            "{condition} shares exit code {code} with another condition"
        );
    }

    assert_eq!(seen[0].0, 0);
    assert_eq!(seen[1].0, 2);
    assert_eq!(seen[2].0, 3);
    assert_eq!(seen[3].0, 4);
    assert_eq!(seen[4].0, 12);
    assert_eq!(seen[5].0, 13);

    // A script author has to be able to find these without experimenting.
    let help = run(&["--help"]).everything();
    for code in ["3", "5", "12", "13"] {
        assert!(
            help.contains(&format!("  {code}   ")) || help.contains(&format!("  {code}  ")),
            "exit code {code} is not documented in --help"
        );
    }
}

/// T3.20 — damage is found, named in full, and exits non-zero.
///
/// Not stopping at the first casualty is half of S-4: the user is deciding
/// whether to go looking for a backup, and that decision needs the whole cost.
#[test]
fn t3_20_damage_is_found_and_every_casualty_named() {
    let scratch = Scratch::new("damage");
    let vault = scratch.vault_arg();
    scratch.with_files(&[
        ("docs", "first.txt", "the first file's content"),
        ("docs", "second.txt", "the second file's content"),
        ("docs", "third.txt", "the third file's content"),
    ]);

    let clean = scratch.veil(&["check", &vault]);
    assert_eq!(clean.code, 0, "{}", clean.everything());
    assert!(clean.out.contains("No damage found"), "{}", clean.out);

    for pack in scratch.packs() {
        scratch.ruin(&pack);
    }

    let damaged = scratch.veil(&["check", &vault]);
    assert_eq!(damaged.code, 5, "{}", damaged.everything());
    for name in ["first.txt", "second.txt", "third.txt"] {
        assert!(
            damaged.out.contains(name),
            "{name} was not named among the damage:\n{}",
            damaged.out
        );
    }
    assert!(
        damaged.out.contains("cannot repair"),
        "the result did not say plainly that Veil2 cannot recover these: {}",
        damaged.out
    );
}

/// T3.21 — a vault already open is reported as in use, not as damage.
#[test]
fn t3_21_a_vault_in_use_says_so() {
    let scratch = Scratch::new("in-use");
    let vault = scratch.vault_arg();
    scratch.with_files(&[("docs", "a.txt", "content")]);

    let held = veil_core::vault::Vault::open(
        &scratch.vault(),
        &veil_core::crypto::Password::new(PASSWORD.to_owned()),
    )
    .unwrap();

    let blocked = scratch.veil(&["list", &vault]);
    assert_eq!(blocked.code, 6, "{}", blocked.everything());
    assert!(
        blocked.err.contains("already open"),
        "the message did not say the vault is open: {}",
        blocked.err
    );

    drop(held);
    assert_eq!(scratch.veil(&["list", &vault]).code, 0);
}

/// T3.23 — a read-only vault reads but does not write.
///
/// Nothing is wrong with it, and the message must not suggest a failing disk:
/// the operation that diagnoses a bad drive has to be the one operation a bad
/// drive can still run.
#[test]
#[cfg(unix)]
fn t3_23_a_read_only_vault_reads_but_does_not_write() {
    use std::os::unix::fs::PermissionsExt;

    let scratch = Scratch::new("read-only");
    let vault = scratch.vault_arg();
    scratch.with_files(&[("docs", "a.txt", "content")]);

    // The lock file is what a read-only medium refuses first.
    let lock = scratch.vault().join("veil.lock");
    std::fs::set_permissions(&lock, std::fs::Permissions::from_mode(0o444)).unwrap();

    assert_eq!(scratch.veil(&["list", &vault]).code, 0, "listing failed");
    assert_eq!(scratch.veil(&["check", &vault]).code, 0, "checking failed");

    let source = scratch.write("sources/new.txt", "more content");
    let refused = scratch.veil(&["add", &vault, source.to_str().unwrap()]);
    assert_eq!(refused.code, 8, "{}", refused.everything());
    assert!(
        refused.err.contains("read-only") && !refused.err.contains("failed"),
        "a read-only vault was reported as a failure: {}",
        refused.err
    );

    std::fs::set_permissions(&lock, std::fs::Permissions::from_mode(0o644)).unwrap();
}

/// T3.24 — a destination file is never overwritten unasked (FR-18).
///
/// The original Veil overwrote silently, and a failed save destroyed the
/// user's only good copy.
#[test]
fn t3_24_an_existing_destination_is_not_overwritten_unasked() {
    let scratch = Scratch::new("overwrite");
    let vault = scratch.vault_arg();
    scratch.with_files(&[("docs", "a.txt", "the stored content")]);

    let destination = scratch.write("existing.txt", "something the user still wants");

    let refused = scratch.veil(&[
        "save-copy",
        &vault,
        "docs/a.txt",
        "--to",
        destination.to_str().unwrap(),
    ]);
    assert_eq!(refused.code, 2, "{}", refused.everything());
    assert!(
        refused.err.contains("existing.txt"),
        "the refusal did not name the file it would have overwritten: {}",
        refused.err
    );
    assert_eq!(
        scratch.read("existing.txt"),
        "something the user still wants"
    );

    let forced = scratch.veil(&[
        "save-copy",
        &vault,
        "docs/a.txt",
        "--to",
        destination.to_str().unwrap(),
        "--force",
    ]);
    assert_eq!(forced.code, 0, "{}", forced.everything());
    assert_eq!(scratch.read("existing.txt"), "the stored content");
}

/// T3.25 — a failed save leaves nothing at the destination (FR-17).
///
/// A truncated plaintext on disk is indistinguishable from a short file, and
/// the user finds out when they need it.
#[test]
fn t3_25_a_failed_save_leaves_no_partial_file() {
    let scratch = Scratch::new("partial");
    let vault = scratch.vault_arg();
    scratch.with_files(&[(
        "docs",
        "big.txt",
        &"content that is long enough to matter. ".repeat(64),
    )]);

    for pack in scratch.packs() {
        scratch.ruin(&pack);
    }

    let destination = scratch.path("recovered.txt");
    let failed = scratch.veil(&[
        "save-copy",
        &vault,
        "docs/big.txt",
        "--to",
        destination.to_str().unwrap(),
    ]);

    assert_eq!(failed.code, 5, "{}", failed.everything());
    assert!(
        !destination.exists(),
        "a partial file was left at the destination"
    );
}

/// T3.26 — adding says the original is still there (FR-9, FR-29).
#[test]
fn t3_26_adding_says_the_original_is_kept() {
    let scratch = Scratch::new("original-kept");
    let vault = scratch.vault_arg();
    assert_eq!(scratch.veil(&["create", &vault]).code, 0);

    let source = scratch.write("sources/a.txt", "content");
    let added = scratch.veil(&["add", &vault, source.to_str().unwrap()]);

    assert_eq!(added.code, 0, "{}", added.err);
    assert!(source.exists(), "the source was moved or deleted");
    assert_eq!(scratch.read("sources/a.txt"), "content");
    assert!(
        added.out.contains("still at") && added.out.contains("did not move or delete"),
        "the retained original was not stated: {}",
        added.out
    );
}

/// T3.27 — deleting says the bytes remain (FR-21, FR-29).
///
/// A user who deletes a file and then hands the vault to someone else must
/// not believe those bytes are gone.
#[test]
fn t3_27_deleting_says_the_bytes_remain() {
    let scratch = Scratch::new("delete-clause");
    let vault = scratch.vault_arg();
    scratch.with_files(&[("docs", "a.txt", "content"), ("docs", "b.txt", "more")]);

    let deleted = scratch.veil(&["delete", &vault, "docs/a.txt"]);
    assert_eq!(deleted.code, 0, "{}", deleted.err);
    assert!(
        deleted.out.contains("stay in the vault"),
        "the persistence of deleted bytes was not stated: {}",
        deleted.out
    );
    assert!(!scratch.veil(&["list", &vault]).out.contains("a.txt"));
}

/// T3.28 — a limit names both numbers (FR-15).
///
/// "Too large" without the two numbers leaves the user to guess what would fit.
#[test]
fn t3_28_a_limit_names_both_numbers() {
    // The per-file limit is 64 GiB, which no test writes. The refusal is
    // provoked through the library, whose message the command line prints
    // verbatim — printing it is the part this phase owns.
    let refusal = veil_core::Error::LimitExceeded {
        limit: veil_core::Limit::FileSize,
        allowed: 64,
        actual: 100,
    }
    .to_string();

    assert!(refusal.contains("64"), "the limit is not named: {refusal}");
    assert!(refusal.contains("100"), "the size is not named: {refusal}");

    let scratch = Scratch::new("limits");
    let vault = scratch.vault_arg();
    scratch.with_files(&[("docs", "a.txt", "content")]);
    let clean = scratch.veil(&["list", &vault]);
    assert_eq!(clean.code, 0, "the fixture itself failed: {}", clean.err);
}
