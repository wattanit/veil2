//! Phase 1 test cases T1.1 through T1.9 — the key hierarchy and the header
//! (HC-3, HC-5, HC-6, HC-7, FR-2, FR-5, FR-6, A-6).
//!
//! Every mutation below is applied to the header's **bytes**, not to a parsed
//! structure. The attacker's position is the file; a suite that mutates
//! through the API tests the API's tolerance of its own values.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use veil_core::crypto::{
    KdfAlgorithm, KdfParams, Password, WRAP_NONCE_LEN, derive_kek, entry_wrap_key,
    generate_master_key, index_key, wrap_master_key,
};
use veil_core::format::{
    CURRENT_FORMAT_VERSION, HEADER_LEN, Header, HeaderError, MAGIC, SALT_LEN, unlock,
};
use veil_core::{Damaged, Error};

const PASSWORD: &str = "a sufficiently long password";

fn password() -> Password {
    Password::new(PASSWORD.to_owned())
}

/// Builds a header exactly as vault creation will, with the parameters given.
fn build_header(params: KdfParams, salt_seed: u8) -> [u8; HEADER_LEN] {
    let salt = [salt_seed; SALT_LEN];
    let nonce = [salt_seed.wrapping_add(1); WRAP_NONCE_LEN];
    let master = generate_master_key();

    let kek = derive_kek(KdfAlgorithm::Argon2id, params, &salt, &password()).unwrap();

    let mut header = Header {
        format_version: CURRENT_FORMAT_VERSION,
        writer_version: [2, 0, 0],
        kdf_algorithm: KdfAlgorithm::Argon2id,
        kdf_params: params,
        kdf_salt: salt,
        wrap_nonce: nonce,
        wrapped_master_key: [0u8; 48],
    };

    // The wrap binds every byte preceding it, so the prefix must be final
    // before the key is wrapped.
    let staged = header.to_bytes();
    let wrapped = wrap_master_key(&kek, &nonce, Header::prefix(&staged), &master).unwrap();
    header.wrapped_master_key = wrapped;
    header.to_bytes()
}

fn test_header() -> [u8; HEADER_LEN] {
    build_header(KdfParams::for_tests(), 0x11)
}

/// T1.1 — key-derivation parameters come from the vault, never from the build
/// (HC-5, Spec §3.1, §4.2).
///
/// Two vaults written with different cost parameters both open, under one
/// build, with no build-side default involved. Changing a default in a later
/// release must never render an existing vault unopenable, and this is the
/// case that fails the moment a constant becomes reachable from the derivation
/// path.
#[test]
fn t1_1_parameters_are_read_from_the_header() {
    let cheap = KdfParams {
        m_cost: 64,
        t_cost: 1,
        p_cost: 1,
    };
    let dearer = KdfParams {
        m_cost: 256,
        t_cost: 3,
        p_cost: 2,
    };
    assert_ne!(cheap, dearer);

    for params in [cheap, dearer] {
        let bytes = build_header(params, 0x22);
        let (header, _) = unlock(&bytes, &password()).expect("vault opens");
        assert_eq!(
            header.kdf_params, params,
            "the header did not round-trip its own parameters"
        );
    }
}

/// T1.2 — a wrong password is reported as a wrong password (FR-2, Spec §6).
#[test]
fn t1_2_wrong_password_is_not_corruption() {
    let bytes = test_header();
    let wrong = Password::new("a different long password".to_owned());

    match unlock(&bytes, &wrong) {
        Err(Error::WrongPassword) => {}
        other => panic!("expected WrongPassword, got {other:?}"),
    }

    // And the right one still opens: the check above must not pass simply
    // because nothing opens.
    unlock(&bytes, &password()).expect("the correct password opens the vault");
}

/// T1.3 — tampering with any header field fails, and fails as damage rather
/// than as a wrong password (HC-3, HC-5, Spec §3.1) — §9 corruption table.
///
/// This is what makes the header need no separate MAC of its own, and it is
/// what closes the parameter-downgrade path: an attacker who lowers the
/// recorded cost does not get a cheaper target, they get a header that no
/// longer authenticates.
#[test]
fn t1_3_tampered_header_fields_fail_as_damage() {
    // Offsets chosen inside each field, from the layout in format::header.
    let cases: &[(&str, usize)] = &[
        ("writer_version", 10),
        ("kdf_algorithm", 16),
        ("m_cost", 18),
        ("t_cost", 22),
        ("p_cost", 26),
        ("kdf_salt", 40),
        ("wrap_nonce", 70),
    ];

    for (field, offset) in cases {
        let mut bytes = test_header();
        bytes[*offset] ^= 0xFF;

        match unlock(&bytes, &password()) {
            Err(Error::Corrupt {
                what: Damaged::Header,
                ..
            }) => {}
            other => panic!("tampering with {field} gave {other:?}, not header damage"),
        }
    }
}

/// T1.3 — tampering with the wrapped key itself fails (HC-3).
///
/// Separated from the fields above because it lies outside the checksummed
/// prefix: nothing but the AEAD stands behind it, which is the point.
#[test]
fn t1_3_tampered_wrapped_key_fails() {
    let mut bytes = test_header();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    assert!(
        unlock(&bytes, &password()).is_err(),
        "a tampered wrapped key opened the vault"
    );
}

/// T1.4 — a file that is not a vault is not a damaged vault (FR-2, §4.2).
#[test]
fn t1_4_not_a_vault_is_distinct_from_damage() {
    let mut wrong_magic = test_header();
    wrong_magic[0] = b'X';
    assert_eq!(Header::parse(&wrong_magic), Err(HeaderError::NotAVault));

    assert_eq!(Header::parse(&[]), Err(HeaderError::NotAVault));
    assert_eq!(Header::parse(&[0u8; 16]), Err(HeaderError::NotAVault));

    // Correct magic but truncated: still not a vault we can read, and still
    // reported before any key derivation is attempted.
    assert_eq!(Header::parse(&MAGIC), Err(HeaderError::NotAVault));
}

/// T1.5 — a too-new format is refused by name (FR-5, §4.2).
#[test]
fn t1_5_too_new_format_is_refused_naming_both_versions() {
    let mut bytes = test_header();
    let ahead = CURRENT_FORMAT_VERSION + 6;
    bytes[8..10].copy_from_slice(&ahead.to_le_bytes());

    match Header::parse(&bytes) {
        Err(HeaderError::TooNew {
            required,
            supported,
        }) => {
            assert_eq!(required, ahead);
            assert_eq!(supported, CURRENT_FORMAT_VERSION);
        }
        other => panic!("expected TooNew, got {other:?}"),
    }

    // And through the crate taxonomy, where the message is assembled.
    match unlock(&bytes, &password()) {
        Err(Error::FormatTooNew {
            required,
            supported,
        }) => {
            let rendered = Error::FormatTooNew {
                required,
                supported,
            }
            .to_string();
            assert!(rendered.contains(&ahead.to_string()));
            assert!(rendered.contains(&CURRENT_FORMAT_VERSION.to_string()));
        }
        other => panic!("expected FormatTooNew, got {other:?}"),
    }
}

/// T1.5 — an unrecognised key-derivation algorithm is refused, never
/// defaulted (HC-5, HC-6).
#[test]
fn t1_5_unknown_kdf_algorithm_is_refused() {
    let mut bytes = test_header();
    bytes[16..18].copy_from_slice(&999u16.to_le_bytes());
    assert!(
        matches!(unlock(&bytes, &password()), Err(Error::Corrupt { .. })),
        "an unknown algorithm identifier did not refuse"
    );
}

/// T1.6 — the writer's version never gates access (HC-5, FR-6).
///
/// Format version and application version have separate lifecycles precisely
/// so that shipping a new release never invalidates a compatibility check. The
/// way that guarantee dies is one well-meant `if writer_version < X`.
#[test]
fn t1_6_writer_version_does_not_gate_access() {
    for writer in [[0, 0, 1], [2, 0, 0], [99, 99, 99]] {
        let params = KdfParams::for_tests();
        let salt = [0x33; SALT_LEN];
        let nonce = [0x44; WRAP_NONCE_LEN];
        let master = generate_master_key();
        let kek = derive_kek(KdfAlgorithm::Argon2id, params, &salt, &password()).unwrap();

        let mut header = Header {
            format_version: CURRENT_FORMAT_VERSION,
            writer_version: writer,
            kdf_algorithm: KdfAlgorithm::Argon2id,
            kdf_params: params,
            kdf_salt: salt,
            wrap_nonce: nonce,
            wrapped_master_key: [0u8; 48],
        };
        let staged = header.to_bytes();
        header.wrapped_master_key =
            wrap_master_key(&kek, &nonce, Header::prefix(&staged), &master).unwrap();
        let bytes = header.to_bytes();

        let (parsed, _) = unlock(&bytes, &password())
            .unwrap_or_else(|e| panic!("writer_version {writer:?} gated access: {e:?}"));
        assert_eq!(parsed.writer_version, writer, "provenance did not survive");
    }
}

/// T1.7 — the master key is generated, not derived (A-6, §3.1).
///
/// Two vaults created with the same password must share nothing. If they did,
/// content keys would be a function of the password, and FR-4's size-
/// independent password change could not exist.
#[test]
fn t1_7_master_key_is_random_not_password_derived() {
    let first = build_header(KdfParams::for_tests(), 0x55);
    let second = build_header(KdfParams::for_tests(), 0x55);

    let (a, master_a) = unlock(&first, &password()).unwrap();
    let (b, master_b) = unlock(&second, &password()).unwrap();

    assert_eq!(a.kdf_salt, b.kdf_salt, "the fixture varied the salt");
    assert_ne!(
        a.wrapped_master_key, b.wrapped_master_key,
        "two vaults with one password produced the same wrapped key"
    );
    assert_ne!(
        master_a.expose(),
        master_b.expose(),
        "the master key is a function of the password"
    );
}

/// T1.8 — subkeys are domain-separated (HC-6, §3.1).
///
/// The original Veil used one key for the header, the metadata, and every
/// file, which is the condition that turns any single nonce mistake into total
/// compromise rather than a local one.
#[test]
fn t1_8_subkeys_are_domain_separated() {
    let master = generate_master_key();
    let index = index_key(&master);
    let wrap = entry_wrap_key(&master);

    assert_ne!(index.expose(), wrap.expose(), "subkeys collide");
    assert_ne!(
        index.expose(),
        master.expose(),
        "index subkey is the master"
    );
    assert_ne!(wrap.expose(), master.expose(), "wrap subkey is the master");

    // Derivation is deterministic: the same master must yield the same
    // subkeys, or a vault would not reopen.
    assert_eq!(index_key(&master).expose(), index.expose());
    assert_eq!(entry_wrap_key(&master).expose(), wrap.expose());

    // And different masters must not.
    let other = generate_master_key();
    assert_ne!(index_key(&other).expose(), index.expose());
}

/// T1.9 — there is exactly one unwrap path (HC-7, §3.1).
///
/// *Honesty clause:* a structural assertion about shape, not a proof that no
/// recovery is possible. What it defends is that the decision cannot be undone
/// by accident — a second wrapping added later would not fit in the header and
/// would fail this test.
#[test]
fn t1_9_the_format_carries_exactly_one_wrapped_key() {
    let bytes = test_header();
    let header = Header::parse(&bytes).unwrap();

    // The header is exactly its fields plus one wrapped key: no spare slot
    // exists in which a second could be placed.
    assert_eq!(bytes.len(), HEADER_LEN);
    assert_eq!(header.wrapped_master_key.len(), 48);

    // Every byte of the header is accounted for by a field, so there is no
    // reserved space an escrow key could later occupy without a format bump.
    let rebuilt = header.to_bytes();
    assert_eq!(
        rebuilt, bytes,
        "the header holds bytes that no field accounts for"
    );
}
