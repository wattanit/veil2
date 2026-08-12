// Whether an entry is eligible for in-app preview (FR-30, C-5) — mirrors
// `preview_entry`'s own two checks (extension, then size) so the context
// menu (P8.4.d) and the preview overlay (P8.6) can decide without a round
// trip. `preview_entry` itself remains the sole authority: this predicate
// only ever shows or hides a menu item, never bypasses the backend's own
// refusal.
//
// A fourth implementation of FR-29's extension-derivation *lookup* — the
// same closed eleven-entry list `preview.rs`'s `classify` holds — kept
// separate from `extensionOf` for the reason that module's own doc gives:
// this answers "is this file one of these known types", not general-purpose
// grouping, so sharing code with the general rule would couple two
// independent decisions to save a few lines.
import type { EntryInfo } from "./api";

export const MAX_PREVIEW_BYTES = 50 * 1024 * 1024;

const PREVIEWABLE_EXTENSIONS = new Set([
  "jpg",
  "jpeg",
  "png",
  "gif",
  "webp",
  "bmp",
  "txt",
  "md",
  "log",
  "csv",
  "json",
]);

export function isPreviewable(entry: EntryInfo): boolean {
  if (entry.size > MAX_PREVIEW_BYTES) {
    return false;
  }
  const dot = entry.name.lastIndexOf(".");
  if (dot <= 0 || dot === entry.name.length - 1) {
    return false;
  }
  const extension = entry.name.slice(dot + 1).toLowerCase();
  return PREVIEWABLE_EXTENSIONS.has(extension);
}
