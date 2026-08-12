//! File extension derivation (FR-29).
//!
//! Implemented independently on the GUI side too
//! (`crates/veil-gui/ui/src/extension.ts`) rather than shared through
//! `veil-core` — Tech Spec §5.1 explains why duplication was chosen over a
//! shared crate for a one-line string operation. `tests/extension_parity.rs`
//! is what keeps the two from drifting: both are checked against the one
//! fixture list there, rather than trusted to agree by construction.

/// The substring of `name` after its last `.`, lowercased for comparison.
/// `None` if `name` has no `.`, if that `.` is the first character (a
/// dotfile has no extension), or if nothing follows it (a trailing dot).
#[must_use]
pub fn extension_of(name: &str) -> Option<String> {
    let dot = name.rfind('.')?;
    if dot == 0 || dot == name.len() - 1 {
        return None;
    }
    Some(name[dot + 1..].to_lowercase())
}
