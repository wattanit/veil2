//! Preview (FR-30, C-5, P7.4): decrypts a supported, in-cap entry to memory
//! and hands it back for display — never to a temporary file, and refused
//! before any ciphertext is read for an entry too large or of an
//! unsupported type. No new dependency: base64 encoding is written out
//! below rather than pulled in (Tech Spec §7's note on this feature round),
//! and no markdown rendering exists at all, per Requirements FR-30.
//!
//! Not wired into the frontend yet — `preview_entry` is reachable from the
//! command layer and tested directly against it; the context menu and
//! overlay that call it are Phase 8's work (Spec §5.3).

use std::io::Cursor;

use tauri::{AppHandle, Manager, Runtime};
use veil_core::{Cancel, EntryId, NoProgress};

use crate::commands::run_blocking;
use crate::errors::ErrorInfo;
use crate::state::AppState;

/// Bytes past which an entry is not offered for preview (Requirements C-5).
/// Decrypting this much into memory costs nothing; the cap bounds preview's
/// own footprint, not a defence against anything (C-5's own rationale).
/// `pub` so `tests/preview.rs` exercises the real boundary rather than a
/// copy of the number that could quietly drift from it.
pub const MAX_PREVIEW_BYTES: u64 = 50 * 1024 * 1024;

/// What `preview_entry` hands back (Tech Spec §5.3).
#[derive(Debug, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum PreviewPayload {
    /// One of the six supported image types.
    Image {
        /// The image's MIME type, for an `<img src="data:{mime};base64,...">`.
        mime: &'static str,
        /// The decrypted image, base64-encoded (§ below on why by hand).
        base64: String,
    },
    /// One of the five supported text types, `.md` included — shown
    /// unrendered, per Requirements FR-30.
    Text {
        /// The decrypted text, decoded as UTF-8.
        content: String,
    },
}

/// How a supported extension is handled — an image, decrypted and shown as
/// one, or text, decrypted and shown unrendered (FR-30's plain-text
/// decision).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Image(&'static str),
    Text,
}

/// FR-30's supported list, exactly: images by their `PreviewPayload::Image`
/// mime type, text extensions all handled the same way. Lookup keys are
/// lowercase; `classify` lowercases before searching.
const SUPPORTED: &[(&str, Kind)] = &[
    ("jpg", Kind::Image("image/jpeg")),
    ("jpeg", Kind::Image("image/jpeg")),
    ("png", Kind::Image("image/png")),
    ("gif", Kind::Image("image/gif")),
    ("webp", Kind::Image("image/webp")),
    ("bmp", Kind::Image("image/bmp")),
    ("txt", Kind::Text),
    ("md", Kind::Text),
    ("log", Kind::Text),
    ("csv", Kind::Text),
    ("json", Kind::Text),
];

/// `name`'s extension, by FR-29's own rule (substring after the last `.`,
/// none for a dotfile or a trailing dot), looked up against [`SUPPORTED`].
/// A third implementation of that one rule, alongside the CLI's and the
/// GUI frontend's (Tech Spec §5.1, §11) — accepted here for the same reason:
/// this is a closed eleven-entry lookup answering "is this file one of
/// these known types", not FR-29's general-purpose grouping, so sharing
/// code between the two would couple two independent decisions for a
/// saving of a few lines.
fn classify(name: &str) -> Option<Kind> {
    let dot = name.rfind('.')?;
    if dot == 0 || dot == name.len() - 1 {
        return None;
    }
    let extension = name[dot + 1..].to_lowercase();
    SUPPORTED
        .iter()
        .find(|(candidate, _)| *candidate == extension)
        .map(|(_, kind)| *kind)
}

fn too_large(size: u64) -> ErrorInfo {
    ErrorInfo {
        kind: "PreviewTooLarge",
        message: format!(
            "this file is {size} bytes; preview is only available up to {MAX_PREVIEW_BYTES} bytes"
        ),
    }
}

fn unsupported() -> ErrorInfo {
    ErrorInfo {
        kind: "PreviewUnsupported",
        message: "this file's type can't be previewed".to_owned(),
    }
}

fn not_text() -> ErrorInfo {
    ErrorInfo {
        kind: "PreviewNotText",
        message: "this file's content isn't valid text".to_owned(),
    }
}

/// Decrypts one entry to memory for preview (FR-30) — never to a temporary
/// file (HC-2). Checked, in this order, before any ciphertext is read: the
/// entry exists, its recorded size is within [`MAX_PREVIEW_BYTES`], and its
/// extension is supported. Only then does [`Vault::extract`] run (Spec
/// §5.1), with `dst` a `Cursor<Vec<u8>>` in place of a file — the same
/// verification (FR-18) a save-copy gets, since this is that same read with
/// its destination held in memory instead.
///
/// Uncancellable and reports no progress: C-5's cap exists specifically so
/// this stays fast enough that neither is missed (Requirements C-5's own
/// reasoning), not because the general obligation (A-3) does not apply.
#[tauri::command]
pub async fn preview_entry<R: Runtime>(
    app: AppHandle<R>,
    id: u64,
) -> Result<PreviewPayload, ErrorInfo> {
    run_blocking(move || {
        app.state::<AppState>().with_vault(|vault| {
            let entry_id = EntryId::new(id);
            let entry = vault
                .entries()
                .iter()
                .find(|e| e.id == entry_id)
                .ok_or_else(|| ErrorInfo::from(veil_core::Error::NotFound))?;

            if entry.size > MAX_PREVIEW_BYTES {
                return Err(too_large(entry.size));
            }
            let kind = classify(&entry.name).ok_or_else(unsupported)?;

            let mut buffer = Cursor::new(Vec::new());
            vault
                .extract(entry_id, &mut buffer, &mut NoProgress, &Cancel::new())
                .map_err(ErrorInfo::from)?;
            let bytes = buffer.into_inner();

            match kind {
                Kind::Image(mime) => Ok(PreviewPayload::Image {
                    mime,
                    base64: base64_encode(&bytes),
                }),
                Kind::Text => {
                    let content = String::from_utf8(bytes).map_err(|_| not_text())?;
                    Ok(PreviewPayload::Text { content })
                }
            }
        })
    })
    .await
}

/// Standard base64 (RFC 4648, with padding). Written out rather than
/// pulled in as a dependency: encoding is all this needs (the frontend
/// never sends bytes back), it is a dozen lines, and its output is checked
/// against RFC 4648's own test vectors below — the same "written out,
/// checked against known values" call `output::stamp` makes in the CLI.
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied();
        let b2 = chunk.get(2).copied();

        out.push(ALPHABET[(b0 >> 2) as usize] as char);
        out.push(ALPHABET[(((b0 & 0x03) << 4) | (b1.unwrap_or(0) >> 4)) as usize] as char);
        if let Some(b1) = b1 {
            out.push(ALPHABET[(((b1 & 0x0F) << 2) | (b2.unwrap_or(0) >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if let Some(b2) = b2 {
            out.push(ALPHABET[(b2 & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::base64_encode;

    /// RFC 4648 §10's own test vectors.
    #[test]
    fn matches_rfc_4648_test_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }
}
