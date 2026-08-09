//! Creating, opening, and re-keying a vault — everything that handles the
//! password (Spec §3.1, §4.2; FR-1, FR-2, FR-4).

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::crypto::{
    KdfAlgorithm, KdfParams, MIN_PASSWORD_CHARS, MasterKey, Password, WRAP_NONCE_LEN, derive_kek,
    entry_wrap_key, generate_master_key, index_key, wrap_master_key,
};
use crate::error::{Error, Result};
use crate::format::{CURRENT_FORMAT_VERSION, Header, SALT_LEN, unlock};
use crate::index::IndexDocument;

use super::{Access, Limits, Vault, VaultLock};

/// Name of the header file within a vault directory (Spec §4.1).
pub const HEADER_FILE: &str = "veil.header";

impl Vault {
    /// Creates a vault at `dir` (FR-1).
    ///
    /// # Errors
    ///
    /// [`Error::Io`], [`Error::VaultInUse`], or a cryptographic failure.
    pub fn create(dir: &Path, password: &Password, params: KdfParams) -> Result<Self> {
        check_length(password)?;
        std::fs::create_dir_all(dir)?;
        // The vault's own directory entry, before anything is put inside it.
        // A bare relative path has an empty parent, which is the current
        // directory and not something to open by that name.
        if let Some(parent) = dir.parent().filter(|p| !p.as_os_str().is_empty()) {
            crate::durable::sync_dir(parent)?;
        }
        let lock = VaultLock::acquire(dir)?;

        let mut kdf_salt = [0u8; SALT_LEN];
        let mut wrap_nonce = [0u8; WRAP_NONCE_LEN];
        fill_random(&mut kdf_salt)?;
        fill_random(&mut wrap_nonce)?;

        let master = generate_master_key();
        let kek = derive_kek(KdfAlgorithm::Argon2id, params, &kdf_salt, password)?;

        let mut header = Header {
            format_version: CURRENT_FORMAT_VERSION,
            writer_version: writer_version(),
            kdf_algorithm: KdfAlgorithm::Argon2id,
            kdf_params: params,
            kdf_salt,
            wrap_nonce,
            wrapped_master_key: [0u8; 48],
        };
        let staged = header.to_bytes();
        header.wrapped_master_key =
            wrap_master_key(&kek, &wrap_nonce, Header::prefix(&staged), &master)?;

        write_header(dir, &header)?;

        let vault = Self::assemble(
            dir.to_path_buf(),
            header,
            master,
            IndexDocument::empty(),
            lock,
        );
        crate::index::write(&vault.dir, &vault.index_key, &vault.document)?;
        Ok(vault)
    }

    /// Opens a vault, decrypting the whole index into memory (FR-7).
    ///
    /// Touches no entry file, so open cost follows entry count and not vault
    /// size (S-2). Never verifies content — that reads everything (FR-26).
    ///
    /// # Errors
    ///
    /// [`Error::NotAVault`], [`Error::FormatTooNew`], [`Error::WrongPassword`],
    /// [`Error::VaultInUse`], or [`Error::Corrupt`].
    pub fn open(dir: &Path, password: &Password) -> Result<Self> {
        let bytes = std::fs::read(dir.join(HEADER_FILE)).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::NotAVault
            } else {
                Error::from(e)
            }
        })?;

        let lock = VaultLock::acquire(dir)?;
        let (header, master) = unlock(&bytes, password)?;
        let key = index_key(&master);
        let document = crate::index::read(dir, &key)?;

        // Header and one index slot, and that is the whole of opening a vault.
        // Nothing is written — a write here would advance the generation that
        // FR-24 detects external change with, so a vault opened from a stale
        // copy would outrank the newer one arriving moments later. Nothing
        // walks `entries/` either, which keeps open time off vault size (S-2).
        Ok(Self::assemble(
            dir.to_path_buf(),
            header,
            master,
            document,
            lock,
        ))
    }

    /// Changes the vault's password (FR-4).
    ///
    /// Only the master key's wrapping changes — no content, no index, no entry
    /// key — so the time it takes does not depend on vault size. The old
    /// password is verified before anything is written.
    ///
    /// # Errors
    ///
    /// [`Error::WrongPassword`] if `old` does not open the vault; otherwise
    /// [`Error::Io`].
    pub fn change_password(
        &mut self,
        old: &Password,
        new: &Password,
        params: KdfParams,
    ) -> Result<()> {
        if self.lock.access() == Access::ReadOnly {
            return Err(Error::ReadOnly);
        }
        check_length(new)?;

        // Verified against what is on disk, not what is in memory: the question
        // is whether the caller knows the password that opens this vault now.
        let bytes = std::fs::read(self.dir.join(HEADER_FILE))?;
        let (_, master) = unlock(&bytes, old)?;

        let mut kdf_salt = [0u8; SALT_LEN];
        let mut wrap_nonce = [0u8; WRAP_NONCE_LEN];
        // Fresh salt and nonce. A reused nonce under a rederivable key is a
        // break, not a weakness.
        fill_random(&mut kdf_salt)?;
        fill_random(&mut wrap_nonce)?;

        let kek = derive_kek(KdfAlgorithm::Argon2id, params, &kdf_salt, new)?;

        let mut header = self.header;
        header.writer_version = writer_version();
        header.kdf_algorithm = KdfAlgorithm::Argon2id;
        header.kdf_params = params;
        header.kdf_salt = kdf_salt;
        header.wrap_nonce = wrap_nonce;
        header.wrapped_master_key = [0u8; 48];
        let staged = header.to_bytes();
        header.wrapped_master_key =
            wrap_master_key(&kek, &wrap_nonce, Header::prefix(&staged), &master)?;

        write_header(&self.dir, &header)?;
        self.header = header;
        Ok(())
    }

    /// Re-reads the index from disk, adopting an external writer's change
    /// (FR-24). The way forward after [`Error::ChangedOnDisk`], without asking
    /// for the password again.
    ///
    /// Entry identifiers held from before are stale afterwards; re-read
    /// [`entries`](Self::entries).
    ///
    /// # Errors
    ///
    /// [`Error::Corrupt`] if the index on disk cannot be read.
    pub fn reload(&mut self) -> Result<()> {
        let document = crate::index::read(&self.dir, &self.index_key)?;
        self.document = document;
        self.document.next_entry_id = next_id_floor(&self.document);
        Ok(())
    }

    fn assemble(
        dir: PathBuf,
        header: Header,
        master: MasterKey,
        mut document: IndexDocument,
        lock: VaultLock,
    ) -> Self {
        document.next_entry_id = next_id_floor(&document);
        Self {
            index_key: index_key(&master),
            entry_wrap_key: entry_wrap_key(&master),
            dir,
            header,
            document,
            limits: Limits::default(),
            lock,
        }
    }
}

/// Refuses a password shorter than C-4's minimum (FR-1).
///
/// Applied where a password is *set*, never where one is offered to open a
/// vault: a vault created under an older minimum must still open, or a rule
/// meant to protect people would lock them out of their own data (HC-5).
fn check_length(password: &Password) -> Result<()> {
    if password.char_count() < MIN_PASSWORD_CHARS {
        return Err(Error::PasswordTooShort {
            minimum: MIN_PASSWORD_CHARS,
        });
    }
    Ok(())
}

/// The stored counter, raised to clear every live entry but never lowered:
/// identifiers are bound into nonces, so a counter that went backwards would
/// reissue one.
fn next_id_floor(document: &IndexDocument) -> u64 {
    let highest = document.entries.iter().map(|e| e.id.get()).max();
    document
        .next_entry_id
        .max(highest.map_or(1, |h| h + 1))
        .max(1)
}

/// Writes the header, replacing any existing one.
///
/// Written beside and renamed over: a failure partway leaves the previous
/// header intact rather than a half-written one (HC-4).
fn write_header(dir: &Path, header: &Header) -> Result<()> {
    let final_path = dir.join(HEADER_FILE);
    let staging = dir.join(format!("{HEADER_FILE}.new"));

    {
        let mut file = std::fs::File::create(&staging)?;
        file.write_all(&header.to_bytes())?;
        file.sync_all()?;
    }
    std::fs::rename(&staging, &final_path)?;
    // The rename is a directory change. Without this the old header can still
    // be the one on disk after a crash, with the new one durable under a name
    // nothing looks for (§4.7, HC-4).
    crate::durable::sync_dir(dir)?;
    Ok(())
}

fn fill_random(buf: &mut [u8]) -> Result<()> {
    getrandom::fill(buf).map_err(|_| Error::Io {
        kind: std::io::ErrorKind::Other,
    })
}

fn writer_version() -> [u16; 3] {
    // Provenance only; never gates access (HC-5).
    let mut parts = env!("CARGO_PKG_VERSION").split('.');
    let mut next = || parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    [next(), next(), next()]
}
