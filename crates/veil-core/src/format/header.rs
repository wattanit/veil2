//! The vault header (Spec §4.2; HC-5, FR-5, FR-30).
//!
//! Plaintext, fixed size, and authenticated as associated data by the
//! master-key unwrap (§3.1). It holds only what a reader needs before it has a
//! key.
//!
//! **Byte order is little-endian, everywhere, stated once here.** HC-8 makes a
//! vault portable across platforms, and a header whose interpretation depends
//! on the writing machine's endianness is not.

use crate::crypto::{KdfAlgorithm, KdfParams, WRAP_NONCE_LEN, WRAPPED_KEY_LEN};

/// Identifies the file as a Veil2 vault header.
pub const MAGIC: [u8; 8] = *b"VEIL2\0\0\0";

/// The format version this release writes.
pub const CURRENT_FORMAT_VERSION: u16 = 1;

/// The oldest format version this release still reads.
///
/// Support is not withdrawn while the migration path of Requirements §2.2
/// remains unbuilt: a release may not refuse a vault it can still read,
/// because there is no other route by which the user's data could be
/// recovered.
pub const OLDEST_SUPPORTED_FORMAT_VERSION: u16 = 1;

/// Length of the salt fed to key derivation.
pub const SALT_LEN: usize = 32;

// Field offsets. The header is fixed size, so these are the format.
const OFF_MAGIC: usize = 0;
const OFF_FORMAT_VERSION: usize = 8;
const OFF_WRITER_VERSION: usize = 10;
const OFF_KDF_ALGORITHM: usize = 16;
const OFF_M_COST: usize = 18;
const OFF_T_COST: usize = 22;
const OFF_P_COST: usize = 26;
const OFF_SALT: usize = 30;
const OFF_WRAP_NONCE: usize = 62;
const OFF_CHECKSUM: usize = 86;

/// Every header byte preceding the wrapped master key.
///
/// This span is the associated data of the master-key unwrap, which is what
/// authenticates the whole header without a separate MAC (§3.1).
pub const HEADER_PREFIX_LEN: usize = 90;

/// Total size of the header on disk.
pub const HEADER_LEN: usize = HEADER_PREFIX_LEN + WRAPPED_KEY_LEN;

/// What went wrong reading a header.
///
/// The three outcomes are kept apart because they send a user to three
/// different places (FR-2, FR-5, FR-30).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderError {
    /// The bytes are not a Veil vault at all: wrong magic, or too short.
    NotAVault,
    /// A Veil vault, but written in a format newer than this release reads.
    TooNew {
        /// Version the vault requires.
        required: u16,
        /// Highest version this release reads.
        supported: u16,
    },
    /// A Veil vault whose format this release no longer reads.
    Superseded {
        /// Version the vault uses.
        version: u16,
    },
    /// A Veil vault whose header is damaged.
    ///
    /// Reported when the header's own checksum does not match its contents.
    Damaged,
}

/// A parsed vault header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// The compatibility gate a reader must satisfy.
    pub format_version: u16,
    /// Application version that last wrote the vault.
    ///
    /// **Provenance only; never gates access** (HC-5). Many application
    /// releases may write one format version, and bumping the application must
    /// never invalidate a compatibility check — which is why nothing in this
    /// crate reads this field to decide anything.
    pub writer_version: [u16; 3],
    /// Which key-derivation function the vault records.
    pub kdf_algorithm: KdfAlgorithm,
    /// The cost parameters the vault records (HC-5).
    pub kdf_params: KdfParams,
    /// Salt fed to key derivation.
    pub kdf_salt: [u8; SALT_LEN],
    /// Nonce used to wrap the master key.
    pub wrap_nonce: [u8; WRAP_NONCE_LEN],
    /// The wrapped master key: 32 key bytes plus a 16-byte tag.
    pub wrapped_master_key: [u8; WRAPPED_KEY_LEN],
}

/// A non-cryptographic checksum over the header's own contents.
///
/// **This is not a security control and is not claimed as one.** An adversary
/// who alters a field can recompute it; HC-3 is enforced by the AEAD, which
/// binds this whole span as associated data and fails regardless.
///
/// It exists to answer a question the AEAD cannot: when the unwrap fails, was
/// the password wrong or is the vault damaged? Without it, altering the salt
/// or the wrap nonce is indistinguishable from a typo, and FR-2 requires those
/// two to send the user to different remedies. Accidental damage — the case
/// FR-2 is actually about — does not recompute anything.
fn checksum(prefix: &[u8]) -> u32 {
    // FNV-1a, 32-bit. Chosen for being four lines rather than a dependency;
    // nothing here needs collision resistance, only change detection.
    let mut hash: u32 = 0x811c_9dc5;
    for byte in prefix {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

impl Header {
    /// Serialises the header to its fixed-size on-disk form.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        out[OFF_MAGIC..OFF_MAGIC + 8].copy_from_slice(&MAGIC);
        out[OFF_FORMAT_VERSION..OFF_FORMAT_VERSION + 2]
            .copy_from_slice(&self.format_version.to_le_bytes());
        for (i, part) in self.writer_version.iter().enumerate() {
            let at = OFF_WRITER_VERSION + i * 2;
            out[at..at + 2].copy_from_slice(&part.to_le_bytes());
        }
        out[OFF_KDF_ALGORITHM..OFF_KDF_ALGORITHM + 2]
            .copy_from_slice(&self.kdf_algorithm.as_u16().to_le_bytes());
        out[OFF_M_COST..OFF_M_COST + 4].copy_from_slice(&self.kdf_params.m_cost.to_le_bytes());
        out[OFF_T_COST..OFF_T_COST + 4].copy_from_slice(&self.kdf_params.t_cost.to_le_bytes());
        out[OFF_P_COST..OFF_P_COST + 4].copy_from_slice(&self.kdf_params.p_cost.to_le_bytes());
        out[OFF_SALT..OFF_SALT + SALT_LEN].copy_from_slice(&self.kdf_salt);
        out[OFF_WRAP_NONCE..OFF_WRAP_NONCE + WRAP_NONCE_LEN].copy_from_slice(&self.wrap_nonce);

        let sum = checksum(&out[..OFF_CHECKSUM]);
        out[OFF_CHECKSUM..OFF_CHECKSUM + 4].copy_from_slice(&sum.to_le_bytes());
        out[HEADER_PREFIX_LEN..].copy_from_slice(&self.wrapped_master_key);
        out
    }

    /// Every header byte preceding the wrapped master key.
    ///
    /// This is what the unwrap binds as associated data.
    #[must_use]
    pub fn prefix(bytes: &[u8; HEADER_LEN]) -> &[u8] {
        &bytes[..HEADER_PREFIX_LEN]
    }

    /// Parses a header, dispatching on the recorded format version.
    ///
    /// The order of checks is the format's compatibility policy: magic first,
    /// so a file that is not a vault is never reported as a damaged one;
    /// version next, so an unreadable format is named rather than guessed at
    /// (FR-5); then the checksum, which separates damage from a wrong
    /// password; then the fields.
    ///
    /// # Errors
    ///
    /// See [`HeaderError`].
    pub fn parse(bytes: &[u8]) -> Result<Self, HeaderError> {
        if bytes.len() < HEADER_LEN || bytes[OFF_MAGIC..OFF_MAGIC + 8] != MAGIC {
            return Err(HeaderError::NotAVault);
        }

        let format_version = read_u16(bytes, OFF_FORMAT_VERSION);
        if format_version > CURRENT_FORMAT_VERSION {
            return Err(HeaderError::TooNew {
                required: format_version,
                supported: CURRENT_FORMAT_VERSION,
            });
        }
        if format_version < OLDEST_SUPPORTED_FORMAT_VERSION {
            return Err(HeaderError::Superseded {
                version: format_version,
            });
        }

        let recorded = read_u32(bytes, OFF_CHECKSUM);
        if recorded != checksum(&bytes[..OFF_CHECKSUM]) {
            return Err(HeaderError::Damaged);
        }

        let kdf_algorithm = KdfAlgorithm::from_u16(read_u16(bytes, OFF_KDF_ALGORITHM))
            .ok_or(HeaderError::Damaged)?;
        let kdf_params = KdfParams {
            m_cost: read_u32(bytes, OFF_M_COST),
            t_cost: read_u32(bytes, OFF_T_COST),
            p_cost: read_u32(bytes, OFF_P_COST),
        };
        if !kdf_params.is_in_range() {
            return Err(HeaderError::Damaged);
        }

        let mut kdf_salt = [0u8; SALT_LEN];
        kdf_salt.copy_from_slice(&bytes[OFF_SALT..OFF_SALT + SALT_LEN]);
        let mut wrap_nonce = [0u8; WRAP_NONCE_LEN];
        wrap_nonce.copy_from_slice(&bytes[OFF_WRAP_NONCE..OFF_WRAP_NONCE + WRAP_NONCE_LEN]);
        let mut wrapped_master_key = [0u8; WRAPPED_KEY_LEN];
        wrapped_master_key.copy_from_slice(&bytes[HEADER_PREFIX_LEN..HEADER_LEN]);

        let mut writer_version = [0u16; 3];
        for (i, part) in writer_version.iter_mut().enumerate() {
            *part = read_u16(bytes, OFF_WRITER_VERSION + i * 2);
        }

        Ok(Self {
            format_version,
            writer_version,
            kdf_algorithm,
            kdf_params,
            kdf_salt,
            wrap_nonce,
            wrapped_master_key,
        })
    }
}
