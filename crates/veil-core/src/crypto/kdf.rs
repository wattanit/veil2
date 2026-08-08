//! Argon2id key-encryption key derivation (Spec §3.1, §4.2; HC-5, HC-6, C-3).
//!
//! Derivation reads its cost parameters and salt from the values handed to it,
//! which come from the vault's header. **Nothing here is reachable as a
//! fallback**: code with constants in reach eventually uses them, and the day
//! it does, every vault written under other settings stops opening.
//!
//! Two sets are named — [`KdfParams::for_tests`] (unreachable in release) and
//! [`KdfParams::for_new_vaults`] — and both are for *creating* a vault, never
//! for opening one.

use argon2::{Algorithm, Argon2, Params, Version};

use super::error::CryptoError;
use super::keys::{KEY_LEN, Kek, Password};

/// The key-derivation function a vault records. A number in the header so it
/// need not be Argon2id forever; an unknown value is refused by name, never
/// defaulted.
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

/// The cost parameters a vault records so it stays openable (HC-5). Normally
/// read from a header; constructing one otherwise means creating a vault.
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
    /// The range in which derivation is known to complete rather than exhaust
    /// memory. Not a strength policy. Values outside it mean a damaged header,
    /// not an unusual one.
    const MIN_M_COST: u32 = 8;
    const MAX_M_COST: u32 = 4 * 1024 * 1024;
    const MIN_T_COST: u32 = 1;
    const MAX_T_COST: u32 = 64;
    const MIN_P_COST: u32 = 1;
    const MAX_P_COST: u32 = 64;

    /// Cheap parameters for tests, compiled out of release builds. A vault
    /// created with these stays weak for life — the parameters live in it.
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
    /// **Unmeasured.** An estimate aimed at C-3's one-second budget, kept as a
    /// working value until there is low-spec hardware to tune on. If a number
    /// moves it is likely the memory cost, downward — a vault that will not
    /// open on a small laptop is worse than a slow derivation on a fast one.
    ///
    /// Changing it orphans nothing: it is used at creation only, and opening
    /// derives from what the vault recorded (HC-5).
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
/// parameters. Every other input comes from the header; nothing is defaulted.
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
