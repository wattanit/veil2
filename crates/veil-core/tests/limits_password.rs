//! Phase 2 test cases T2.28 through T2.33 — limits and password change
//! (FR-2, FR-4, FR-16, A-6, C-1, C-2, HC-4, Spec §3.1).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use std::io::Read;

use harness::{add, create, other_password, password, pattern};
use veil_core::crypto::{KdfParams, Password};
use veil_core::vault::{Limits, Vault};
use veil_core::{Cancel, Error, Limit, NoProgress};

/// A source that reports one size and yields another.
///
/// The point of C-2's enforcement: a limit read from file metadata is a limit
/// on files, not on content. This is every non-file source, and a growing file
/// besides.
struct Liar {
    remaining: usize,
}

impl Read for Liar {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = buf.len().min(self.remaining);
        for (i, slot) in buf[..n].iter_mut().enumerate() {
            *slot = (i % 251) as u8;
        }
        self.remaining -= n;
        Ok(n)
    }
}

/// T2.28 — the entry limit is refused by name (FR-16, C-1).
///
/// "Too many files" without the numbers leaves the user unable to act.
#[test]
fn t2_28_the_entry_limit_is_refused_by_name() {
    let scratch = harness::Scratch::new("entry-limit");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir);
    vault.set_limits(Limits {
        max_entries: 3,
        ..Limits::default()
    });

    for i in 0..3 {
        add(&mut vault, &format!("f{i}.bin"), "d", &pattern(100));
    }
    let generation = vault.generation();
    let stats = vault.statistics();
    let files = harness::snapshot(&dir);

    let refused = vault.add(
        "one-too-many.bin",
        "d",
        &mut pattern(100).as_slice(),
        &mut NoProgress,
        &Cancel::new(),
    );

    match refused {
        Err(Error::LimitExceeded {
            limit,
            allowed,
            actual,
        }) => {
            assert_eq!(limit, Limit::EntriesPerVault);
            assert_eq!(allowed, 3);
            assert_eq!(actual, 4);
        }
        other => panic!("expected a named entry-limit refusal, got {other:?}"),
    }

    // Nothing was consumed by the refusal.
    assert_eq!(vault.generation(), generation);
    assert_eq!(vault.statistics(), stats);
    assert_eq!(vault.entries().len(), 3);
    assert_eq!(harness::snapshot(&dir), files);
}

/// T2.29 — the file-size limit is enforced against the stream, not the claim
/// (FR-16, C-2).
#[test]
fn t2_29_the_file_size_limit_is_enforced_against_the_stream() {
    let scratch = harness::Scratch::new("size-limit");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir);
    vault.set_limits(Limits {
        max_file_size: 4096,
        ..Limits::default()
    });

    add(&mut vault, "within.bin", "d", &pattern(4096));
    let generation = vault.generation();
    let stats = vault.statistics();
    let non_entry_files = |dir: &std::path::Path| {
        harness::snapshot(dir)
            .into_iter()
            .filter(|(name, _)| !name.ends_with(".entry"))
            .collect::<Vec<_>>()
    };
    let before = non_entry_files(&dir);

    // A source that is not a file at all, so there is no metadata to read a
    // size from, and one that yields far more than the limit.
    let refused = vault.add(
        "over.bin",
        "d",
        &mut Liar { remaining: 40_000 },
        &mut NoProgress,
        &Cancel::new(),
    );

    match refused {
        Err(Error::LimitExceeded {
            limit,
            allowed,
            actual,
        }) => {
            assert_eq!(limit, Limit::FileSize);
            assert_eq!(allowed, 4096);
            assert!(actual > allowed, "the actual value must exceed the limit");
        }
        other => panic!("expected a named file-size refusal, got {other:?}"),
    }

    assert_eq!(vault.generation(), generation, "a generation was consumed");
    assert_eq!(vault.statistics(), stats);
    assert!(vault.entries().iter().all(|e| e.name != "over.bin"));
    // The header, both index slots, and every other entry's file are
    // untouched. The refused write's own entry file may exist as unreferenced
    // residue (Spec §4.5) — nothing here rolls that back, by design.
    assert_eq!(
        non_entry_files(&dir),
        before,
        "a refused addition changed something other than its own residue"
    );
    harness::assert_statistics_correct(&vault, "after a refused addition");

    // The limit applies to replace as well, or it is a limit on one route in.
    assert!(matches!(
        vault.replace(
            "d",
            "within.bin",
            &mut Liar { remaining: 40_000 },
            &mut NoProgress,
            &Cancel::new(),
        ),
        Err(Error::LimitExceeded {
            limit: Limit::FileSize,
            ..
        })
    ));
    assert_eq!(
        harness::read_back(&vault, vault.find("d", "within.bin").unwrap().id).unwrap(),
        pattern(4096)
    );
}

/// T2.30 — a new password opens the vault and the old one no longer does
/// (FR-4, FR-2).
///
/// Content extracting unchanged is what proves the master key survived the
/// rewrap.
#[test]
fn t2_30_a_new_password_opens_the_vault_and_the_old_does_not() {
    let scratch = harness::Scratch::new("password-change");
    let dir = scratch.vault_dir();

    let mut vault = create(&dir);
    let content = pattern(6000);
    let id = add(&mut vault, "doc.bin", "d", &content);

    let new = other_password("one");
    vault
        .change_password(&password(), &new, KdfParams::for_tests())
        .unwrap();

    // Still usable through the open handle: a password change is not a close.
    assert_eq!(harness::read_back(&vault, id).unwrap(), content);
    drop(vault);

    assert!(matches!(
        Vault::open(&dir, &password()),
        Err(Error::WrongPassword)
    ));

    let reopened = Vault::open(&dir, &new).expect("the new password opens it");
    assert_eq!(reopened.entries().len(), 1);
    assert_eq!(harness::read_back(&reopened, id).unwrap(), content);
}

/// T2.31 — password change touches only the header (FR-4, A-6).
///
/// FR-4's size-independence follows from this structurally, which is a stronger
/// statement than a timing measurement on shared CI hardware.
#[test]
fn t2_31_password_change_touches_only_the_header() {
    let scratch = harness::Scratch::new("password-scope");
    let dir = scratch.vault_dir();

    let mut vault = create(&dir);
    for i in 0..5 {
        add(&mut vault, &format!("f{i}.bin"), "d", &pattern(3000));
    }
    let generation = vault.generation();
    let before = harness::snapshot(&dir);

    vault
        .change_password(&password(), &other_password("two"), KdfParams::for_tests())
        .unwrap();
    let after = harness::snapshot(&dir);

    let changed: Vec<&String> = before
        .keys()
        .filter(|name| before.get(*name) != after.get(*name))
        .collect();
    assert_eq!(
        changed,
        vec!["veil.header"],
        "a password change touched more than the header"
    );
    assert_eq!(
        after.len(),
        before.len(),
        "a password change left an extra file behind: {:?}",
        after
            .keys()
            .filter(|k| !before.contains_key(*k))
            .collect::<Vec<_>>()
    );
    // No index generation is consumed either: nothing about the index changed.
    assert_eq!(vault.generation(), generation);
}

/// T2.32 — two changes in a row both take effect (FR-4).
///
/// A rewrap that reuses the salt, or wraps under a stale key-encryption key,
/// passes a single-change test.
#[test]
fn t2_32_two_changes_in_a_row_both_take_effect() {
    let scratch = harness::Scratch::new("password-twice");
    let dir = scratch.vault_dir();

    let mut vault = create(&dir);
    let id = add(&mut vault, "doc.bin", "d", &pattern(1000));

    let first = other_password("first");
    let second = other_password("second");
    let salt_zero = vault.header().kdf_salt;

    vault
        .change_password(&password(), &first, KdfParams::for_tests())
        .unwrap();
    let salt_one = vault.header().kdf_salt;

    vault
        .change_password(&first, &second, KdfParams::for_tests())
        .unwrap();
    let salt_two = vault.header().kdf_salt;
    drop(vault);

    // A fresh salt each time. Reusing one would make two passwords' wrappings
    // relatable.
    assert_ne!(salt_zero, salt_one);
    assert_ne!(salt_one, salt_two);
    assert_ne!(salt_zero, salt_two);

    assert!(matches!(
        Vault::open(&dir, &password()),
        Err(Error::WrongPassword)
    ));
    assert!(matches!(
        Vault::open(&dir, &first),
        Err(Error::WrongPassword)
    ));
    let vault = Vault::open(&dir, &second).expect("the newest password opens it");
    assert_eq!(harness::read_back(&vault, id).unwrap(), pattern(1000));
}

/// T2.33 — a wrong old password changes nothing (FR-4, FR-2, HC-4).
///
/// Verifying after writing would destroy a vault on a typo.
#[test]
fn t2_33_a_wrong_old_password_changes_nothing() {
    let scratch = harness::Scratch::new("password-wrong");
    let dir = scratch.vault_dir();

    let mut vault = create(&dir);
    let id = add(&mut vault, "doc.bin", "d", &pattern(1000));
    let before = harness::snapshot(&dir);

    let attempt = vault.change_password(
        &Password::new("not the current password at all".to_owned()),
        &other_password("three"),
        KdfParams::for_tests(),
    );
    assert!(
        matches!(attempt, Err(Error::WrongPassword)),
        "expected WrongPassword, got {attempt:?}"
    );

    assert_eq!(
        harness::snapshot(&dir),
        before,
        "a refused password change wrote to the vault"
    );
    drop(vault);

    let vault = Vault::open(&dir, &password()).expect("the original password still opens it");
    assert_eq!(harness::read_back(&vault, id).unwrap(), pattern(1000));
}
