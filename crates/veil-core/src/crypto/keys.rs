//! Key material types (Spec §3.1).
//!
//! Every type here is a distinct newtype, so that a subkey cannot be passed
//! where a master key is expected. Type confusion in a key hierarchy is a
//! silent catastrophe and the type system is free.
//!
//! Every type is zeroised on drop, prints a placeholder under `Debug`, and
//! implements no `Display`, no `Clone`, and no serialisation. HC-2 forbids key
//! material reaching any error or debug output, and a derived `Debug` is the
//! ordinary way that happens.
//!
//! *Honesty clause:* zeroisation is a type-level obligation, not an observed
//! erasure. Confirming that freed memory was cleared is not possible in safe
//! Rust and not portable across the three supported platforms. Spec §3.4
//! already declines to defend against memory capture on a running machine, so
//! nothing downstream rests on a stronger claim than this.

use zeroize::{Zeroize, ZeroizeOnDrop};

/// Length of every key in the hierarchy, in bytes.
pub const KEY_LEN: usize = 32;

macro_rules! secret_key {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        ///
        /// Zeroised on drop. `Debug` prints a placeholder (HC-2).
        #[derive(ZeroizeOnDrop)]
        pub struct $name([u8; KEY_LEN]);

        impl $name {
            #[doc = concat!("Takes ownership of raw key bytes as a `", stringify!($name), "`.")]
            #[must_use]
            pub const fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
                Self(bytes)
            }

            /// Borrows the raw key bytes.
            ///
            /// Every call site that uses this is a place where key material
            /// escapes its type, so there should be few and they should be
            /// inside this module's own hierarchy.
            #[must_use]
            pub fn expose(&self) -> &[u8; KEY_LEN] {
                &self.0
            }
        }

        impl core::fmt::Debug for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(concat!(stringify!($name), "(<redacted>)"))
            }
        }
    };
}

secret_key! {
    /// Key-encryption key, derived from the password with Argon2id over the
    /// header's recorded parameters. Never stored.
    Kek
}

secret_key! {
    /// The vault's master key: 32 bytes from the OS CSPRNG at creation, never
    /// derived from the password (A-6). Stored only in wrapped form, and with
    /// exactly one unwrap path (HC-7).
    MasterKey
}

secret_key! {
    /// Subkey protecting the index, derived from the master key by HKDF with a
    /// versioned `info` string.
    IndexKey
}

secret_key! {
    /// Subkey wrapping each entry's data key, derived from the master key by
    /// HKDF with a versioned `info` string distinct from the index subkey's.
    EntryWrapKey
}

secret_key! {
    /// One entry's data key, generated at ingest and stored wrapped in the
    /// index (Spec §3.2).
    Dek
}

/// A password held for the lifetime of a derivation and no longer.
///
/// Zeroised on drop. `Debug` prints a placeholder (HC-2). The core never
/// prompts for one — it is a parameter, which is what makes A-1 true.
#[derive(ZeroizeOnDrop)]
pub struct Password(Vec<u8>);

impl Password {
    /// Takes ownership of a password's bytes.
    #[must_use]
    pub fn new(mut password: String) -> Self {
        let bytes = password.as_bytes().to_vec();
        password.zeroize();
        Self(bytes)
    }

    /// Borrows the password bytes for derivation.
    #[must_use]
    pub fn expose(&self) -> &[u8] {
        &self.0
    }

    /// Number of bytes, for the minimum-length check of C-4.
    ///
    /// Length is the only credential policy Veil2 applies. Strength estimation
    /// is a promise about an attacker's resources the product does not make.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the password is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Length in characters, for C-4.
    ///
    /// Characters rather than bytes: C-4 says twelve characters, and a Thai or
    /// Han password reaches twelve bytes in four. Falls back to bytes when the
    /// password is not valid UTF-8 — a password read from a file need not be,
    /// and refusing it for that would be a policy C-4 does not state.
    #[must_use]
    pub fn char_count(&self) -> usize {
        core::str::from_utf8(&self.0).map_or(self.0.len(), |s| s.chars().count())
    }
}

/// The minimum password length C-4 fixes, in characters.
///
/// Enforced here rather than in each application: two frontends applying their
/// own minimum is how one of them ends up with a weaker one (A-4).
pub const MIN_PASSWORD_CHARS: usize = 12;

impl core::fmt::Debug for Password {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Password(<redacted>)")
    }
}
