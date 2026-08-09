//! Unicode NFC normalisation of stored names (Spec §4.6; HC-8, FR-13).
//!
//! Applied wherever a `name` or `folder` is captured from a caller or from a
//! filesystem walk, and again wherever one is used to *match* an existing
//! entry — Spec §4.6 fixes comparison as exact "after normalisation," not
//! only storage as normalised.

use std::borrow::Cow;

use unicode_normalization::{IsNormalized, UnicodeNormalization, is_nfc_quick};

/// `s`, normalised to NFC.
///
/// Borrows rather than allocates when `s` already is one — true for nearly
/// everything, since most text typed directly or read from a non-Apple
/// filesystem already arrives pre-composed. `is_nfc_quick` answering "maybe"
/// is treated the same as "no": either way the safe route is the full pass.
pub(super) fn nfc(s: &str) -> Cow<'_, str> {
    match is_nfc_quick(s.chars()) {
        IsNormalized::Yes => Cow::Borrowed(s),
        IsNormalized::No | IsNormalized::Maybe => Cow::Owned(s.nfc().collect()),
    }
}
