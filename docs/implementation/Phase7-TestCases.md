# Veil2 — Phase 7 Test Cases: Capability Surface

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-12
**Owner:** wattanit
**Foundation and plan versions these cases are built against:**
- Requirements Document v1.1 — upstream
- Design Guideline v1.3 — upstream
- Technical Specification v1.1 — upstream
- Implementation Plan v1.0 — upstream
- [Phase7-ToDo.md](Phase7-ToDo.md) v1.0 — companion; each case names the item it covers

This document owns the enumerated checks that close Phase 7. Every case cites the requirement it verifies.

---

## Conventions

**Case identifiers** are `T7.<n>`, sequential within this document.

**Two kinds of case.** T7.1 through T7.3 and T7.5 through T7.7 and T7.16 run the built `veil` binary as a subprocess, the same way Phase 3's cases do. T7.8 through T7.15 call the Tauri commands directly through `tauri::test::mock_builder`, the way Phase 5 established for command-layer logic that needs no real window. T7.4 is its own kind again: a Rust test that also shells out to `node` to run the TypeScript half.

**Every CLI case asserts the exit code**, not merely that the command failed.

**Nothing here reads a terminal.**

**Where these run.** The development machine, macOS.

**How to run them.**

```bash
cargo test --release -p veil-cli    # T7.1, T7.2, T7.3, T7.5, T7.6, T7.7, T7.16
cargo test -p veil-gui              # T7.4, T7.8–T7.15
```

`T7.4` additionally requires `node` on `PATH` — already a build-time requirement of `crates/veil-gui/ui`'s own `npm run build` (esbuild), so nothing new is added to run it.

---

## Per-entry detail

### T7.1 — `veil detail` reports full metadata
*Covers P7.1.b, P7.1.d, P7.6.a · Verifies FR-28, Design §8.9*

Add a file, then run `detail` on it.
**Verdict:** the table shows name, folder, exact byte size, `Modified` (the source's own mtime), and `Added`. No content-hash line appears anywhere in the output.

### T7.2 — `detail`'s JSON carries the same facts
*Covers P7.1.c, P7.1.d, P7.6.a · Verifies FR-28, Design §3.4*

Run `detail --format json` on the same file.
**Verdict:** parses, the same fields as the table, size as an exact integer, no hash field present.

### T7.3 — `detail` on an unknown path names nothing, not damage
*Covers P7.1.e, P7.6.a · Verifies FR-28, FR-2, HC-3*

Ask for detail on a path never added.
**Verdict:** exit 13 (`NotFound`), a message saying no file matches, distinct from the damage code and the wrong-password code.

---

## Extension derivation

### T7.4 — Both implementations agree on the written rule
*Covers P7.2.a, P7.2.b, P7.2.c, P7.2.d · Verifies FR-29*

Run the Rust function and, via `node`, the TypeScript function against one shared fixture list: `archive.tar.gz` → `gz`, `.gitignore` → none, `README` → none, `photo.JPG` → `jpg`, `file.` → none, `a.b.c` → `c`, `IMG_1.png` → `png`.
**Verdict:** identical output from both implementations for every case in the list.

---

## The `--group` flag

### T7.5 — `--group=extension` groups the listing
*Covers P7.3.b, P7.3.c, P7.6.b · Verifies FR-29, Design §3.2*

Add files of several extensions, including one with none, then `list --group=extension` in table and JSON.
**Verdict:** one group per distinct extension, lowercased, the extensionless file under its own reserved group, in both output modes.

### T7.6 — Bare `--group`'s table output is unchanged; its JSON output is not
*Covers P7.3.a, P7.3.c, P7.6.b · Verifies FR-29, A-4*

Run `list --group` (table) against a vault with entries in several folders; compare its output against a recording of the same command taken before this phase's change. Then run `list --group --format json` against the same vault.
**Verdict:** the table is byte-identical to the pre-change recording. The JSON is **not** identical to before — it now groups (`{"groups": [...]}`) where it previously always printed a flat `{"files": [...]}` regardless of `--group`. This is the one documented exception to "no script observes any difference" (Spec §5.2): the prior JSON behavior was never documented or tested as intentional, so this is a gap closed, not a behavior broken.

### T7.7 — Omitted `--group` stays flat
*Covers P7.3.a, P7.6.b · Verifies FR-29*

Run `list` with no `--group` flag at all, against the same vault.
**Verdict:** a flat listing, unchanged from before this phase.

---

## Preview

### T7.8 — A supported, in-cap entry previews correctly
*Covers P7.4.a, P7.4.b, P7.4.c, P7.4.d · Verifies FR-30, C-5*

Add a small `.png` and a small `.txt`, call `preview_entry` on each through the mock Tauri harness.
**Verdict:** `Image { mime: "image/png", base64 }` decodes back to the original bytes; `Text { content }` matches the original string exactly. Observe and record the base64 payload's size against C-5's cap (the Plan's open item).

### T7.9 — An unsupported extension is refused without reading ciphertext
*Covers P7.4.b, P7.5.b · Verifies FR-30*

Add a `.exe` file whose stored ciphertext is then deliberately corrupted (would fail FR-18 if ever read), call `preview_entry`.
**Verdict:** refused, naming the extension as unsupported; the corruption is never surfaced, proving the entry's file was never opened.

### T7.10 — An entry above C-5 is refused without reading ciphertext
*Covers P7.4.a, P7.5.b · Verifies FR-30, C-5*

Add a `.txt` file just over C-5's cap, corrupt its stored ciphertext, call `preview_entry`.
**Verdict:** refused, naming the cap; the corruption is never surfaced.

### T7.11 — Invalid UTF-8 in a text-listed extension is refused, not garbled
*Covers P7.4.e · Verifies FR-30*

Add a `.txt` file whose plaintext is not valid UTF-8, call `preview_entry`.
**Verdict:** a typed refusal. No `Text` payload is returned with replacement characters or truncated content standing in for the original bytes.

### T7.12 — A failed integrity check during preview is reported, not passed through
*Covers P7.4.c · Verifies FR-18, FR-30*

Add a small, supported, in-cap file; corrupt its stored ciphertext; call `preview_entry`.
**Verdict:** the same failure `extract` reports for a damaged entry. No payload is returned.

---

## Preview's guarantees

### T7.13 — A successful preview touches no file in the vault directory
*Covers P7.5.a · Verifies FR-30, HC-2*

Snapshot every file's name, size, and mtime under the vault directory; call `preview_entry` successfully (T7.8's case); snapshot again.
**Verdict:** identical. Nothing created, removed, or modified.

### T7.14 — A refused or failed preview touches no file either
*Covers P7.5.a · Verifies FR-30, HC-2*

Repeat T7.13's snapshot around each of T7.9, T7.10, and T7.12's calls.
**Verdict:** identical in every case.

---

## Disclosure

### T7.15 — No preview path discloses content, name, or folder beyond what extraction already logs
*Covers P7.7.a, P7.7.b · Verifies HC-1, HC-2, Spec §6*

Add a file whose content and whose name both contain a distinctive marker. Call `preview_entry` for the success case (T7.8), the refusal cases (T7.9, T7.10), and the failure case (T7.12), capturing every error value, `Debug` output, and `tracing` line produced.
**Verdict:** the content marker never appears anywhere. The name marker appears only where an entry's name already appears for every other command today (the returned error's caller-supplied context, if any) — never in a log line.

---

## Vocabulary

### T7.16 — New CLI strings hold the fixed vocabulary

**Superseded — no separate case.** `detail`'s and `--group`'s help text and refusal messages are checked by extending `tests/audits.rs`'s existing T3.31 (vocabulary) and T3.32 (disclosure) to scan them, rather than by a second scanner built to do the same thing (P7.6.c). One real finding this surfaced: the `GroupBy` enum's own doc comments, which clap turns into `--help`'s "Possible values" text, said "each entry's..." — caught by T3.31, fixed to "each file's...". `preview_entry`'s own refusal messages (unsupported extension, over-cap, invalid text) are not yet coverable here — they do not exist until P7.4 — and will be added to the same two audits' scan lists when Preview is built, rather than reopening this case number for them.

---

## Coverage

The GUI's own use of extension derivation, and every frontend-visible consequence of `preview_entry` (the overlay, its wording, its memory clearing), are Phase 8's coverage — nothing here substitutes for it. FR-24 is not provoked from the CLI, per Phase 3's own coverage note; nothing in this phase changes that.
