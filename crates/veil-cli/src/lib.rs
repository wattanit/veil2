//! A narrow library surface over an otherwise-binary crate.
//!
//! `veil-cli` is a binary, not a library (A-4 does not require it to be
//! one — parity is proved through the built binary, per every existing test
//! in `tests/`). This file exists solely so `extension_of` (P7.2) is
//! reachable from an integration test without spawning a subprocess for a
//! pure string function; nothing else is exported here, and nothing else
//! should need to be.
pub mod extension;
