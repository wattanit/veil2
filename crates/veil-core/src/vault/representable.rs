//! Whether a stored name can become a filename outside the vault without
//! becoming a different name than the vault reports (Spec §4.6; FR-31,
//! HC-8).
//!
//! The rules enforced are the **union** of every supported platform's, not
//! only the host platform's: a name Windows reserves is refused here on
//! macOS too. The alternative — dispatching on the host platform — would
//! make this check's own answer a host fact, which is exactly what HC-8
//! exists to keep out of Veil2. It also means P5.3's fixture and P8.2's
//! cross-platform run need one expectation per name, not one per platform.

use crate::error::{Error, Result, Unrepresentable};
use crate::index::EntryId;

use super::Vault;

/// Windows' reserved device names, compared case-insensitively against a
/// name with any extension removed.
const RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Characters no supported platform allows in a filename, beyond control
/// characters (checked separately): Windows' reserved set, `/` and NUL for
/// every POSIX filesystem, and `\` because it is Windows' own separator.
const RESERVED_CHARACTERS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*', '\u{0}'];

/// `name` alone, without regard to what else the vault holds.
fn check_name(name: &str) -> core::result::Result<(), Unrepresentable> {
    if name.ends_with('.') || name.ends_with(' ') {
        return Err(Unrepresentable::TrailingDotOrSpace);
    }
    if name
        .chars()
        .any(|c| RESERVED_CHARACTERS.contains(&c) || c.is_control())
    {
        return Err(Unrepresentable::ReservedCharacter);
    }
    let stem = name.split('.').next().unwrap_or(name);
    if RESERVED_NAMES.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        return Err(Unrepresentable::ReservedName);
    }
    Ok(())
}

/// Whether two names collide on a case-insensitive destination.
///
/// Full Unicode case folding, via [`str::to_lowercase`] — the fixture's
/// non-Latin scripts have no case at all, and the ASCII shortcut a
/// case-insensitive *filesystem* does not use would miss the Latin, Greek and
/// Cyrillic pairs that do.
fn case_collides(a: &str, b: &str) -> bool {
    a != b && a.to_lowercase() == b.to_lowercase()
}

impl Vault {
    /// Whether `id`'s name can become a filename here (Spec §4.6, FR-31).
    ///
    /// Checked against the name alone, and against every other entry in the
    /// same folder for a case collision — never against what happens to be on
    /// disk at a destination right now, which a first extraction would find
    /// empty and miss entirely.
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`] if no entry has that identifier, or
    /// [`Error::NameNotRepresentable`] naming why.
    pub fn check_representable(&self, id: EntryId) -> Result<()> {
        let Some(entry) = self.document.entries.iter().find(|e| e.id == id) else {
            return Err(Error::NotFound);
        };

        if let Err(reason) = check_name(&entry.name) {
            return Err(Error::NameNotRepresentable { id, reason });
        }

        let collides = self.document.entries.iter().any(|other| {
            other.id != id
                && other.folder == entry.folder
                && case_collides(&other.name, &entry.name)
        });
        if collides {
            return Err(Error::NameNotRepresentable {
                id,
                reason: Unrepresentable::CaseCollision,
            });
        }

        Ok(())
    }
}
