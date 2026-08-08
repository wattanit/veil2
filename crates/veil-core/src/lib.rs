//! Veil2 core: vault format, cryptography, storage, and the vault API.
//!
//! This library has no interactive input or output and assumes neither a
//! terminal nor a graphical shell (A-1). Credentials are passed in as
//! parameters; nothing here prompts. The command-line and graphical
//! applications are peer consumers holding presentation logic only (A-4).
//!
//! Module ownership follows Technical Specification §1.

pub mod crypto;
pub mod error;
pub mod format;
pub mod index;
pub mod store;
pub mod vault;

pub use error::{Damaged, Error, Limit, Result};
pub use index::EntryId;
