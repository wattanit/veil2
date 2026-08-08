//! Phase 0 test cases T0.5 and T0.6 — key material (HC-2, Spec §3.1, §6).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use veil_core::crypto::{Dek, EntryWrapKey, IndexKey, Kek, MasterKey, Password};
use zeroize::ZeroizeOnDrop;

/// A byte pattern distinctive enough that a partial disclosure is visible.
const PATTERN: [u8; 32] = [
    0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE, 0xF0, 0x0D, 0xFA, 0xCE, 0xD0, 0x0D, 0xFE, 0xED,
    0x8B, 0xAD, 0xF0, 0x0D, 0x1B, 0xAD, 0xB0, 0x02, 0xB1, 0x6B, 0x00, 0xB5, 0x0B, 0x00, 0xB1, 0x35,
];

/// Every way key bytes could be spelled in a formatted string.
///
/// Checking only the raw bytes would pass a `Debug` implementation that
/// helpfully hex-dumps the key, which is the likeliest form the leak takes.
fn encodings_of(bytes: &[u8]) -> Vec<String> {
    let lower: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let upper: String = bytes.iter().map(|b| format!("{b:02X}")).collect();
    let decimal_list = bytes
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let debug_array = format!("{bytes:?}");
    vec![lower, upper, decimal_list, debug_array]
}

fn assert_redacted(type_name: &str, rendered: &str, secret: &[u8]) {
    assert!(
        rendered.contains("<redacted>"),
        "{type_name}: Debug output does not redact: {rendered}"
    );
    for (i, encoding) in encodings_of(secret).into_iter().enumerate() {
        assert!(
            !rendered.contains(&encoding),
            "{type_name}: Debug output discloses key material (encoding {i})"
        );
    }
    // A four-byte prefix is enough to notice a truncated dump.
    let prefix: String = secret.iter().take(4).map(|b| format!("{b:02x}")).collect();
    assert!(
        !rendered.contains(&prefix),
        "{type_name}: Debug output discloses a prefix of the key material"
    );
}

/// T0.5 — no key type discloses its bytes (HC-2).
#[test]
fn t0_5_key_types_redact_under_debug() {
    assert_redacted("Kek", &format!("{:?}", Kek::from_bytes(PATTERN)), &PATTERN);
    assert_redacted(
        "MasterKey",
        &format!("{:?}", MasterKey::from_bytes(PATTERN)),
        &PATTERN,
    );
    assert_redacted(
        "IndexKey",
        &format!("{:?}", IndexKey::from_bytes(PATTERN)),
        &PATTERN,
    );
    assert_redacted(
        "EntryWrapKey",
        &format!("{:?}", EntryWrapKey::from_bytes(PATTERN)),
        &PATTERN,
    );
    assert_redacted("Dek", &format!("{:?}", Dek::from_bytes(PATTERN)), &PATTERN);
}

/// T0.5 — the password type discloses nothing either (HC-2).
///
/// The password is not key material in the hierarchy's sense, but it is the
/// one secret whose loss is unrecoverable by design (HC-7), so it is held to
/// the same rule.
#[test]
fn t0_5_password_redacts_under_debug() {
    let secret = "correct horse battery staple";
    let password = Password::new(secret.to_owned());
    let rendered = format!("{password:?}");
    assert!(rendered.contains("<redacted>"));
    assert!(
        !rendered.contains(secret),
        "Password Debug output discloses the password"
    );
    assert!(
        !rendered.contains("horse"),
        "Password Debug output discloses part of the password"
    );
}

/// T0.6 — every key type carries the zeroisation obligation (Spec §3.1).
///
/// A compile-time assertion: adding a key type without the bound fails to
/// compile rather than failing a test that someone might not have written.
///
/// *Honesty clause:* this proves the obligation is carried, not that memory
/// was cleared. Observing freed memory is neither possible in safe Rust nor
/// portable across the three supported platforms, and Spec §3.4 already
/// declines to defend against memory capture on a running machine — so
/// nothing downstream rests on a stronger claim than this test makes.
#[test]
fn t0_6_key_types_are_zeroised_on_drop() {
    const fn requires_zeroize_on_drop<T: ZeroizeOnDrop>() {}

    requires_zeroize_on_drop::<Kek>();
    requires_zeroize_on_drop::<MasterKey>();
    requires_zeroize_on_drop::<IndexKey>();
    requires_zeroize_on_drop::<EntryWrapKey>();
    requires_zeroize_on_drop::<Dek>();
    requires_zeroize_on_drop::<Password>();
}

/// T0.5 — key types are distinct, so one cannot stand in for another.
///
/// Type confusion in a key hierarchy is silent and catastrophic. This asserts
/// the property by construction: the function below only compiles because each
/// type is its own, and it would stop compiling if any two were aliased.
#[test]
fn t0_5_key_types_are_not_interchangeable() {
    const fn distinct<T>() -> core::any::TypeId
    where
        T: 'static,
    {
        core::any::TypeId::of::<T>()
    }

    let ids = [
        distinct::<Kek>(),
        distinct::<MasterKey>(),
        distinct::<IndexKey>(),
        distinct::<EntryWrapKey>(),
        distinct::<Dek>(),
    ];
    for (i, a) in ids.iter().enumerate() {
        for b in ids.iter().skip(i + 1) {
            assert_ne!(a, b, "two key roles share one type");
        }
    }
}
