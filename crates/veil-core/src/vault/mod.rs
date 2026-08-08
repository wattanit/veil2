//! Public API, orchestration, locking, progress, and cancellation
//! (Spec §2, §5.1).
//!
//! **Phase 1 scope only.** This is the vertical slice of Plan task P1.10 — the
//! first point at which header, key hierarchy, index persistence, packs, and
//! content encryption are proven to compose rather than to work individually.
//!
//! Not here yet, and scheduled rather than forgotten: advisory locking
//! (FR-26), progress and cancellation (A-3), replace and delete (FR-13,
//! FR-21), statistics maintenance (FR-22), limit enforcement (FR-15), password
//! change (FR-4), and verification (FR-33). All are Phase 2.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::crypto::{
    Dek, EntryWrapKey, IndexKey, KdfAlgorithm, KdfParams, MasterKey, NONCE_PREFIX_LEN, Password,
    WRAP_NONCE_LEN, decrypt, derive_kek, encrypt, entry_wrap_key, generate_dek,
    generate_master_key, generate_nonce_prefix, index_key, unwrap_dek, wrap_dek, wrap_master_key,
};
use crate::error::{Error, Result};
use crate::format::{CURRENT_FORMAT_VERSION, Header, SALT_LEN, unlock};
use crate::index::{Entry, EntryId, IndexDocument, Statistics};
use crate::store::{DEFAULT_PACK_CAP, PackSink, PackSource};

/// Name of the header file within a vault directory (Spec §4.1).
pub const HEADER_FILE: &str = "veil.header";

/// An open vault.
///
/// **An instance value, not a singleton** (A-7). Nothing here is process
/// global, so the single-vault limit stays a product decision rather than a
/// structural one, and supporting several open vaults later is a caller-side
/// change.
pub struct Vault {
    dir: PathBuf,
    header: Header,
    index_key: IndexKey,
    entry_wrap_key: EntryWrapKey,
    document: IndexDocument,
    pack_cap: u64,
}

impl Vault {
    /// Creates a vault at `dir`.
    ///
    /// `pack_cap` is a parameter rather than a constant so that multi-pack
    /// behaviour — spanning, and the damage locality of S-4 — is testable
    /// without gigabytes of fixture.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the directory cannot be written, or a cryptographic
    /// failure during setup.
    pub fn create(
        dir: &Path,
        password: &Password,
        params: KdfParams,
        pack_cap: u64,
    ) -> Result<Self> {
        std::fs::create_dir_all(dir)?;

        let mut kdf_salt = [0u8; SALT_LEN];
        let mut wrap_nonce = [0u8; WRAP_NONCE_LEN];
        getrandom::fill(&mut kdf_salt).map_err(|_| Error::Io {
            kind: std::io::ErrorKind::Other,
        })?;
        getrandom::fill(&mut wrap_nonce).map_err(|_| Error::Io {
            kind: std::io::ErrorKind::Other,
        })?;

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

        std::fs::write(dir.join(HEADER_FILE), header.to_bytes())?;

        let vault = Self::assemble(
            dir.to_path_buf(),
            header,
            &master,
            IndexDocument::empty(),
            pack_cap,
        );
        crate::index::write(&vault.dir, &vault.index_key, &vault.document)?;
        Ok(vault)
    }

    /// Opens a vault.
    ///
    /// # Errors
    ///
    /// [`Error::NotAVault`], [`Error::FormatTooNew`], [`Error::WrongPassword`],
    /// or [`Error::Corrupt`].
    pub fn open(dir: &Path, password: &Password) -> Result<Self> {
        let bytes = std::fs::read(dir.join(HEADER_FILE)).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::NotAVault
            } else {
                Error::from(e)
            }
        })?;

        let (header, master) = unlock(&bytes, password)?;
        let key = index_key(&master);
        let document = crate::index::read(dir, &key)?;
        Ok(Self::assemble(
            dir.to_path_buf(),
            header,
            &master,
            document,
            DEFAULT_PACK_CAP,
        ))
    }

    /// Sets the pack cap for subsequent writes.
    ///
    /// Reads follow whatever extents an entry already records, so changing
    /// this never invalidates stored content.
    pub fn set_pack_cap(&mut self, cap: u64) {
        self.pack_cap = cap;
    }

    fn assemble(
        dir: PathBuf,
        header: Header,
        master: &MasterKey,
        document: IndexDocument,
        pack_cap: u64,
    ) -> Self {
        Self {
            dir,
            header,
            index_key: index_key(master),
            entry_wrap_key: entry_wrap_key(master),
            document,
            pack_cap,
        }
    }

    /// The complete index, served from memory (FR-6).
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.document.entries
    }

    /// The vault's totals (FR-8).
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        self.document.statistics
    }

    /// The parsed header, for provenance and diagnostics.
    #[must_use]
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Stores one file's content under the given name and folder.
    ///
    /// **Ordering is what makes FR-12 true.** Content is written and fsynced
    /// before the index generation that references it advances. A crash
    /// between the two leaves pack bytes that no index references — garbage,
    /// reclaimed by the reconciliation of Phase 4 — and never an index entry
    /// pointing at bytes that were not durable.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] or a cryptographic failure.
    pub fn add(&mut self, name: &str, folder: &str, src: &mut impl Read) -> Result<EntryId> {
        let id = self.next_entry_id();
        let dek = generate_dek();
        let nonce_prefix = generate_nonce_prefix();

        let mut sink = PackSink::open(&self.dir, self.pack_cap)?;
        let summary = encrypt(&dek, &nonce_prefix, id.get(), src, &mut sink)?;
        let extents = sink.finish()?;

        let entry = Entry {
            id,
            name: name.to_owned(),
            folder: folder.to_owned(),
            size: summary.plaintext_len,
            source_mtime: now(),
            added_at: now(),
            content_hash: summary.hash,
            wrapped_dek: wrap_dek(&self.entry_wrap_key, id.get(), &dek)?,
            nonce_prefix,
            extents,
            unknown: std::collections::BTreeMap::new(),
        };

        self.document.entries.push(entry);
        self.document.statistics.entry_count = self.document.entries.len() as u64;
        self.document.statistics.logical_bytes += summary.plaintext_len;
        self.document.statistics.physical_bytes += summary.ciphertext_len;
        self.document.generation += 1;

        crate::index::write(&self.dir, &self.index_key, &self.document)?;
        Ok(id)
    }

    /// Writes one entry's content to `dst`, verified.
    ///
    /// Nothing reaches `dst` before it has authenticated, and the content hash
    /// is compared after the final chunk (FR-17).
    ///
    /// # Errors
    ///
    /// [`Error::Corrupt`] naming the entry when the content is damaged.
    pub fn extract(&self, id: EntryId, dst: &mut impl Write) -> Result<()> {
        let Some(entry) = self.document.entries.iter().find(|e| e.id == id) else {
            return Err(Error::Corrupt {
                what: crate::error::Damaged::Content,
                affected: vec![id],
            });
        };

        let dek: Dek = unwrap_dek(&self.entry_wrap_key, id.get(), &entry.wrapped_dek)?;
        let mut source = PackSource::new(&self.dir, &entry.extents);

        let prefix: [u8; NONCE_PREFIX_LEN] = entry.nonce_prefix;
        decrypt(
            &dek,
            &prefix,
            id.get(),
            Some(&entry.content_hash),
            &mut source,
            dst,
        )
        .map(|_| ())
        .map_err(|e| match e {
            crate::crypto::CryptoError::ContentHashMismatch => Error::Corrupt {
                what: crate::error::Damaged::ContentHash,
                affected: vec![id],
            },
            _ => Error::Corrupt {
                what: crate::error::Damaged::Content,
                affected: vec![id],
            },
        })
    }

    fn next_entry_id(&self) -> EntryId {
        // Identifiers are never reused: a reused id would let a wrapped key
        // from a deleted entry decrypt under a live one's nonce.
        let highest = self.document.entries.iter().map(|e| e.id.get()).max();
        EntryId::new(highest.map_or(1, |h| h + 1))
    }
}

fn writer_version() -> [u16; 3] {
    // Provenance only; never gates access (HC-5).
    let mut parts = env!("CARGO_PKG_VERSION").split('.');
    let mut next = || parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    [next(), next(), next()]
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
