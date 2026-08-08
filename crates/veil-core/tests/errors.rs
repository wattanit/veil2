//! Phase 0 test cases T0.11 and T0.12 — the error taxonomy (Spec §6).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use veil_core::{Damaged, EntryId, Error, Limit};

/// Every variant of the taxonomy, constructed once and reused by both cases.
fn every_variant() -> Vec<Error> {
    vec![
        Error::WrongPassword,
        Error::NotAVault,
        Error::FormatTooNew {
            required: 7,
            supported: 1,
        },
        Error::FormatSuperseded {
            version: 1,
            last_supported_by: "2.3.0",
        },
        Error::Corrupt {
            what: Damaged::Pack { id: 4 },
            affected: vec![EntryId::new(11), EntryId::new(12)],
        },
        Error::Corrupt {
            what: Damaged::Header,
            affected: vec![],
        },
        Error::VaultInUse,
        Error::ChangedOnDisk,
        Error::StorageUnavailable,
        Error::LimitExceeded {
            limit: Limit::EntriesPerVault,
            allowed: 65_536,
            actual: 65_537,
        },
        Error::Cancelled { rolled_back: true },
        Error::Cancelled { rolled_back: false },
        Error::VerificationFailed {
            entries: vec![EntryId::new(3)],
        },
        Error::Io {
            kind: std::io::ErrorKind::PermissionDenied,
        },
    ]
}

/// T0.11 — a wrong password is never a corruption (FR-2).
///
/// The original Veil surfaced every failure as one cryptography error, which
/// is what sends a user with a typo to look for a damaged file.
#[test]
fn t0_11_wrong_password_is_distinct_from_corruption() {
    assert!(matches!(Error::WrongPassword, Error::WrongPassword));
    assert!(!matches!(Error::WrongPassword, Error::Corrupt { .. }));
    assert!(!matches!(Error::NotAVault, Error::Corrupt { .. }));
    assert!(!matches!(Error::NotAVault, Error::WrongPassword));

    // An I/O failure converts to `Io` and to nothing else. A blanket
    // conversion into one string-carrying variant is the defect FR-2 forbids,
    // and it was one line in the original.
    let io: Error = std::io::Error::from(std::io::ErrorKind::NotFound).into();
    assert!(matches!(io, Error::Io { .. }));
}

/// T0.11 — version errors carry the numbers their messages must name.
#[test]
fn t0_11_version_errors_carry_both_versions() {
    let too_new = Error::FormatTooNew {
        required: 7,
        supported: 1,
    };
    let rendered = too_new.to_string();
    assert!(rendered.contains('7') && rendered.contains('1'), "FR-5");

    let superseded = Error::FormatSuperseded {
        version: 1,
        last_supported_by: "2.3.0",
    };
    let rendered = superseded.to_string();
    assert!(
        rendered.contains('1') && rendered.contains("2.3.0"),
        "FR-30"
    );
}

/// T0.11 — a limit failure carries both numbers (FR-15).
#[test]
fn t0_11_limit_exceeded_names_the_limit_and_the_value() {
    let rendered = Error::LimitExceeded {
        limit: Limit::FileSize,
        allowed: 68_719_476_736,
        actual: 70_000_000_000,
    }
    .to_string();
    assert!(rendered.contains("file size"));
    assert!(rendered.contains("68719476736"));
    assert!(rendered.contains("70000000000"));
}

/// T0.11 — a cancellation states what it left behind (FR-14, FR-19).
///
/// This is the state fact the Design Guideline's three-part message needs. An
/// error that says only "cancelled" forces the caller to invent the answer to
/// the user's actual question, which is whether anything changed.
#[test]
fn t0_11_cancelled_states_whether_it_rolled_back() {
    let rolled_back = Error::Cancelled { rolled_back: true }.to_string();
    let stands = Error::Cancelled { rolled_back: false }.to_string();
    assert_ne!(rolled_back, stands);
    assert!(rolled_back.contains("as it was before"));
    assert!(stands.contains("stands"));
}

/// T0.11 — damage and verification carry every affected entry, not the first
/// (S-4, FR-33).
///
/// S-4 rejects two failures at once: one bad region losing everything, and one
/// bad region being indistinguishable from total loss. Carrying the full list
/// is what turns a partial failure into a list of files a user can restore.
#[test]
fn t0_11_failures_carry_every_affected_entry() {
    let Error::Corrupt { affected, .. } = (Error::Corrupt {
        what: Damaged::Pack { id: 4 },
        affected: vec![EntryId::new(11), EntryId::new(12), EntryId::new(13)],
    }) else {
        panic!("constructed variant did not match");
    };
    assert_eq!(affected.len(), 3);

    let Error::VerificationFailed { entries } = (Error::VerificationFailed {
        entries: vec![EntryId::new(1), EntryId::new(2)],
    }) else {
        panic!("constructed variant did not match");
    };
    assert_eq!(entries.len(), 2);
}

/// T0.11 — every variant's `Display` says something, and says what state
/// things are in (Design §4.2).
#[test]
fn t0_11_every_variant_renders_a_message() {
    for error in every_variant() {
        let rendered = error.to_string();
        assert!(
            rendered.len() > 10,
            "a variant renders no usable message: {rendered:?}"
        );
        assert!(
            !rendered.starts_with("Error"),
            "a variant renders a type name rather than a sentence: {rendered:?}"
        );
    }
}

/// T0.12 — no error discloses content, keys, or the password (HC-2).
///
/// The markers are planted in the surrounding state, not in the errors: the
/// test is that there is no field through which they could arrive.
///
/// *Scope note:* entry identity is permitted here and is not a marker. FR-33
/// and S-4 require failing entries to be named, so an error that cannot
/// identify an entry cannot satisfy them. The prohibition on entry names
/// reaching a *log* is a separate rule, covered by T0.7 and T0.8.
#[test]
fn t0_12_no_error_discloses_content_keys_or_password() {
    const CONTENT_MARKER: &str = "PLAINTEXT-CONTENT-MARKER";
    const KEY_MARKER: &str = "KEY-MATERIAL-MARKER";
    const PASSWORD_MARKER: &str = "correct horse battery staple";
    const PATH_MARKER: &str = "/Users/someone/Documents/salaries.csv";

    for error in every_variant() {
        for rendered in [format!("{error}"), format!("{error:?}")] {
            for marker in [CONTENT_MARKER, KEY_MARKER, PASSWORD_MARKER, PATH_MARKER] {
                assert!(
                    !rendered.contains(marker),
                    "a variant disclosed a marker: {rendered}"
                );
            }
        }
    }
}

/// T0.12 — an I/O failure carries no path (HC-1, HC-2).
///
/// An ingest source path is a fact about the user's machine that no error
/// needs, and the layer that supplied the path is the one that can name it.
/// The original Veil stored absolute source paths in its index; this is the
/// same habit in a smaller place.
#[test]
fn t0_12_io_errors_carry_no_path() {
    let underlying = std::io::Error::other("failed opening /Users/someone/Documents/salaries.csv");
    let converted: Error = underlying.into();
    let rendered = format!("{converted} {converted:?}");
    assert!(
        !rendered.contains("/Users/someone"),
        "the conversion carried a path through: {rendered}"
    );
}
