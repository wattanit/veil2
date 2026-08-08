//! Argon2id key-encryption key derivation (Spec §3.1, §4.2; HC-5, HC-6, C-3).
//!
//! **There is no default parameter set in this module, and that is the point.**
//! Every derivation reads its cost parameters and salt from the values it is
//! handed, which come from the vault's header. Code that reads recorded
//! parameters and also has constants within reach will eventually use the
//! constants, and the day it does, every vault written under different
//! settings becomes unopenable — the HC-5 failure the original Veil was one
//! edit away from, having hardcoded its Argon2 constants.
//!
//! Two parameter sets are named: [`KdfParams::for_tests`], which a release
//! build cannot reach, and [`KdfParams::for_new_vaults`], which is what a
//! *creation* uses. Neither is reachable from the derivation path, and that is
//! the distinction that matters — a value chosen when a vault is made is not a
//! fallback for opening one.

use argon2::{Algorithm, Argon2, Params, Version};

use super::error::CryptoError;
use super::keys::{KEY_LEN, Kek, Password};

/// Identifies the key-derivation function a vault records.
///
/// Stored in the header as a number so that it need not be Argon2id forever
/// (Spec §4.2). An unrecognised value is refused by name rather than falling
/// back to a default — a fallback here would derive a key with settings the
/// vault never agreed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum KdfAlgorithm {
    /// Argon2id, the only algorithm this release derives with.
    Argon2id,
}

impl KdfAlgorithm {
    /// The value recorded in a header for this algorithm.
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        match self {
            Self::Argon2id => 1,
        }
    }

    /// Reads a recorded value, or `None` when this release does not know it.
    #[must_use]
    pub const fn from_u16(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::Argon2id),
            _ => None,
        }
    }
}

/// The cost parameters a vault records so that it stays openable (HC-5).
///
/// Held as a value read from a header. Constructing one from anything else is
/// creating a new vault's parameters, which happens in exactly one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    /// Memory cost, in kibibytes.
    pub m_cost: u32,
    /// Time cost, in passes.
    pub t_cost: u32,
    /// Degree of parallelism, in lanes.
    pub p_cost: u32,
}

impl KdfParams {
    /// Bounds on what this build will run.
    ///
    /// These are not a policy about strength — they are the range within which
    /// the derivation is known to complete rather than exhaust memory or run
    /// unboundedly. A header carrying values outside them is damaged, not
    /// merely unusual, which is what lets a caller tell tampering from a wrong
    /// password.
    const MIN_M_COST: u32 = 8;
    const MAX_M_COST: u32 = 4 * 1024 * 1024;
    const MIN_T_COST: u32 = 1;
    const MAX_T_COST: u32 = 64;
    const MIN_P_COST: u32 = 1;
    const MAX_P_COST: u32 = 64;

    /// Cheap parameters for tests.
    ///
    /// Compiled out of release builds. A vault created with these is a weak
    /// vault, and HC-5 means the parameters live in it permanently — so a leak
    /// here is not a slow build, it is a vault that stays weak for its whole
    /// life.
    #[cfg(any(test, debug_assertions))]
    #[must_use]
    pub const fn for_tests() -> Self {
        Self {
            m_cost: 64,
            t_cost: 1,
            p_cost: 1,
        }
    }

    /// The parameters a newly created vault records (C-3, Spec §11.1).
    ///
    /// **These have not been measured, and the Specification says so.** They
    /// are the estimate of Spec §11.1 — chosen to approach C-3's one-second
    /// budget while staying feasible on a modest machine — accepted by the
    /// owner as a working value until there is low-spec hardware to tune on. A
    /// vault that cannot be opened on a small laptop is a worse failure than a
    /// slow derivation on a fast one, so if either number moves it is likely
    /// this one, downward.
    ///
    /// **Changing it orphans nothing** (HC-5). It is used at creation only;
    /// opening a vault derives from what that vault recorded, and this constant
    /// is unreachable from that path. A caller is free to pass something else.
    #[must_use]
    pub const fn for_new_vaults() -> Self {
        Self {
            // 256 MiB, expressed in the kibibytes the parameter is measured in.
            m_cost: 256 * 1024,
            t_cost: 3,
            p_cost: 4,
        }
    }

    /// Whether these parameters are within the range this build will run.
    #[must_use]
    pub const fn is_in_range(self) -> bool {
        self.m_cost >= Self::MIN_M_COST
            && self.m_cost <= Self::MAX_M_COST
            && self.t_cost >= Self::MIN_T_COST
            && self.t_cost <= Self::MAX_T_COST
            && self.p_cost >= Self::MIN_P_COST
            && self.p_cost <= Self::MAX_P_COST
    }
}

/// Derives the key-encryption key from a password and a vault's recorded
/// parameters.
///
/// Every input other than the password comes from the header. Nothing is
/// defaulted, and nothing is inferred.
///
/// # Errors
///
/// Fails when the parameters are outside the range this build runs, or when
/// the derivation itself cannot proceed.
pub fn derive_kek(
    algorithm: KdfAlgorithm,
    params: KdfParams,
    salt: &[u8],
    password: &Password,
) -> Result<Kek, CryptoError> {
    if !params.is_in_range() {
        return Err(CryptoError::ParametersOutOfRange);
    }

    let KdfAlgorithm::Argon2id = algorithm;

    let params = Params::new(params.m_cost, params.t_cost, params.p_cost, Some(KEY_LEN))
        .map_err(|_| CryptoError::ParametersOutOfRange)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut out = [0u8; KEY_LEN];
    argon
        .hash_password_into(password.expose(), salt, &mut out)
        .map_err(|_| CryptoError::Derivation)?;
    Ok(Kek::from_bytes(out))
}
