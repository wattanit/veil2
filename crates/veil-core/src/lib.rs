//! Veil2 core: vault format, cryptography, storage, and the vault API.
//!
//! No interactive input or output, and no assumption of a terminal or a
//! graphical shell (A-1). Credentials are parameters; nothing here prompts.
//!
//! For a walk-through, run `cargo run -p veil-core --example demo`.

#![forbid(unsafe_code)]

pub mod crypto;
pub mod durable;
pub mod error;
pub mod format;
pub mod index;
pub mod store;
pub mod vault;

pub use error::{Damaged, Error, Limit, Result};
pub use index::EntryId;
pub use vault::{
    Cancel, Limits, NoProgress, Progress, ProgressReport, Reclaimed, Reconciled, Unit, Vault,
};
