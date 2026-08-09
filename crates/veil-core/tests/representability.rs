//! Phase 5 test cases T5.5 through T5.8 — extraction representability
//! (Spec §4.6; FR-31, HC-8).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use harness::{SMALL_CAP, add, create, pattern};
use veil_core::{Error, Unrepresentable};

/// T5.5 — A reserved device name is refused, not silently altered
/// (FR-31, HC-8).
#[test]
fn t5_5_a_reserved_device_name_is_refused() {
    let scratch = harness::Scratch::new("representable-reserved");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);
    let id = add(&mut vault, "CON.txt", "", &pattern(10));

    let err = vault.check_representable(id).unwrap_err();
    assert!(matches!(
        err,
        Error::NameNotRepresentable {
            id: failing,
            reason: Unrepresentable::ReservedName,
        } if failing == id
    ));
}

/// T5.6 — Every name in the reserved set is refused, and nothing outside
/// it is (FR-31).
#[test]
fn t5_6_every_reserved_name_is_refused_and_nothing_else_is() {
    let scratch = harness::Scratch::new("representable-set");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);

    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM9", "LPT1", "LPT9", "con.txt", "Nul.log",
    ];
    for name in reserved {
        let id = add(&mut vault, name, "reserved", &pattern(5));
        assert!(
            matches!(
                vault.check_representable(id),
                Err(Error::NameNotRepresentable {
                    reason: Unrepresentable::ReservedName,
                    ..
                })
            ),
            "{name} should have been refused as a reserved name"
        );
    }

    let colon = add(&mut vault, "10:30am.txt", "reserved", &pattern(5));
    assert!(matches!(
        vault.check_representable(colon),
        Err(Error::NameNotRepresentable {
            reason: Unrepresentable::ReservedCharacter,
            ..
        })
    ));
    let star = add(&mut vault, "notes*.txt", "reserved", &pattern(5));
    assert!(matches!(
        vault.check_representable(star),
        Err(Error::NameNotRepresentable {
            reason: Unrepresentable::ReservedCharacter,
            ..
        })
    ));

    let allowed = ["CONSOLE.txt", "CON-fig.txt", "report (final).pdf"];
    for name in allowed {
        let id = add(&mut vault, name, "fine", &pattern(5));
        assert!(
            vault.check_representable(id).is_ok(),
            "{name} should not have been refused — a reserved *prefix* is not a reserved name"
        );
    }
}

/// T5.7 — A case collision is refused (FR-31, HC-8).
///
/// A case-sensitive destination could hold both; the check is deliberately
/// conservative rather than dependent on which filesystem happens to receive
/// the copy (Spec §4.6).
#[test]
fn t5_7_a_case_collision_is_refused() {
    let scratch = harness::Scratch::new("representable-case");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);

    let upper = add(&mut vault, "Photo.jpg", "pics", &pattern(10));
    let lower = add(&mut vault, "photo.jpg", "pics", &pattern(20));

    for id in [upper, lower] {
        assert!(matches!(
            vault.check_representable(id),
            Err(Error::NameNotRepresentable {
                reason: Unrepresentable::CaseCollision,
                ..
            })
        ));
    }

    // A same-cased name in a different folder never collides.
    let elsewhere = add(&mut vault, "photo.jpg", "other", &pattern(30));
    assert!(vault.check_representable(elsewhere).is_ok());
}

/// T5.8 — The collision check looks at the vault, not at what is already
/// on disk (Spec §4.6, HC-8).
///
/// There is nothing on disk to collide with here at all — the point is that
/// the refusal comes from the vault's own two entries, not from probing a
/// destination.
#[test]
fn t5_8_the_collision_check_looks_at_the_vault_not_the_disk() {
    let scratch = harness::Scratch::new("representable-vault-not-disk");
    let dir = scratch.vault_dir();
    let mut vault = create(&dir, SMALL_CAP);

    let a = add(&mut vault, "Photo.jpg", "pics", &pattern(10));
    let b = add(&mut vault, "photo.jpg", "pics", &pattern(20));

    assert!(vault.check_representable(a).is_err());
    assert!(vault.check_representable(b).is_err());
}
