# Veil2 — Phase 8 Test Cases: Browsing Screen

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-12
**Owner:** wattanit
**Foundation and plan versions these cases are built against:**
- Requirements Document v1.1 — upstream
- Design Guideline v1.3 — upstream
- Technical Specification v1.1 — upstream
- Implementation Plan v1.0 — upstream
- [Phase8-ToDo.md](Phase8-ToDo.md) v1.0 — companion; each case names the item it covers

This document owns the enumerated checks that close Phase 8. Every case cites the requirement it verifies.

---

## Conventions

**Case identifiers** are `T8.<n>`, sequential within this document.

**Three kinds of case**, extending Phase 6's own distinction (its Conventions §) with the kind Phase 7 introduced:
1. **Pure frontend logic** — sort comparators (`ui/src/sort.ts`) and selection-transition logic (`ui/src/selection.ts`), bundled with the frontend's own `esbuild` and run under `node`, the same mechanism `crates/veil-cli/tests/extension_parity.rs` (T7.4) established. Unlike T7.4, there is no second, independent implementation to check agreement against — these cases assert fixed expected output for chosen inputs.
2. **Rendering and interaction** — layout, wording, menus, collapse/expand, dialogs — checked live in `cargo tauri dev`, by a person looking at it, per Phase 5 and Phase 6's own conventions. This project has no browser/DOM automation harness; none is introduced for this phase.
3. **Static scans** — vocabulary and config checks over source and manifests, the same class of check `tests/vocabulary.rs`, `tests/structure.rs`, and `tests/audits.rs` already run.

**Where these run.** The development machine, macOS.

**How to run them.**

```bash
cargo test -p veil-gui              # T8.4, T8.7, T8.24 (automated)
cargo tauri dev                     # everything else — followed live against this checklist
```

`T8.4` and `T8.7` additionally require `node` on `PATH`, already a build-time requirement of `crates/veil-gui/ui`'s own `npm run build`.

---

## Grouping

### T8.1 — Three-way grouping renders none, folder, and extension views
*Covers P8.1.a, P8.1.b · Verifies FR-8, FR-29, Design §3.2*

Open a vault with files of several extensions, including one with none, in more than one folder. Select each of the grouping control's three options in turn.
**Verdict:** "none" shows a flat list; "folder" groups by folder exactly as before this phase; "extension" groups by lowercased extension, with the extensionless file under its own reserved group. No boolean checkbox remains anywhere in the controls bar.

### T8.2 — Collapsing a group hides its rows but keeps its count visible
*Covers P8.1.c · Verifies Design §3.2*

Collapse one group among several, in both the folder and extension groupings.
**Verdict:** the collapsed group's rows are hidden; its header still shows its row count; the other groups are unaffected; the collapsed state persists across an unrelated action (e.g., search) within the same session.

### T8.3 — Changing grouping or a lock/reopen cycle expands every group
*Covers P8.1.d · Verifies Design §3.2*

Collapse a group, then switch the grouping control to a different option; separately, collapse a group, then lock and reopen the same vault.
**Verdict:** in both cases, every group is expanded afterward — no group remembers being collapsed across either change.

---

## Sort

### T8.4 — Comparators sort correctly per column, ascending and descending
*Covers P8.2.b, P8.2.c · Verifies Design §3.2*

Run `sort.ts`'s four comparators (name, folder, size, added) under `node`, via the esbuild-bundle-and-shell mechanism T7.4 established, against a fixture list including mixed-case names, equal sizes, and equal timestamps.
**Verdict:** name and folder sort case-insensitively; size and added sort numerically; each comparator's reversed form (for descending) produces the exact reverse of its ascending output, including a stable order for equal keys.

### T8.5 — Clicking a header cycles ascending, then descending; a different header resets
*Covers P8.2.a, P8.2.b, P8.2.d · Verifies Design §3.2*

Click the `Name` header once, then again; then click `Size`.
**Verdict:** first click sorts ascending by name with an ascending arrow shown beside `Name`; second click reverses to descending, arrow flips; clicking `Size` sorts ascending by size, with the arrow now beside `Size` and gone from `Name`. No separate sort control exists in the controls bar.

### T8.6 — Sort composes with grouping: within each group, not across the list
*Covers P8.2.c · Verifies Design §3.2*

With extension grouping active and more than one group present, sort by size descending.
**Verdict:** rows within each group are ordered largest-first; the groups themselves are not reordered by this sort, and a row in one group is never interleaved with rows from another.

---

## Multi-select

### T8.7 — Selection-transition logic computes the correct set for each click kind
*Covers P8.3.a, P8.3.b · Verifies Design §3.2*

Run `selection.ts`'s pure transition function under `node` against a fixed visual order and a sequence of click kinds: plain click on row 3; shift-click on row 7 (expect rows 3–7 selected); Cmd-click on row 5 (expect row 5 removed, 3–4 and 6–7 remaining); plain click on row 1 (expect only row 1 selected).
**Verdict:** each step's resulting set matches exactly what is described above.

### T8.8 — Delete acts on the entire multi-selection
*Covers P8.3.c · Verifies Design §3.2, §4.1*

Select three rows (one plain click, one shift-click extension) and invoke Delete.
**Verdict:** the confirmation names the count (3) as existing dialogs already do (Design §4.1); on confirming, all three are removed and none of the untouched rows are affected.

### T8.9 — Replace… is unavailable outside a single-row selection
*Covers P8.3.d · Verifies Design §3.2, §8.7*

Select zero, one, then three rows; check the Replace… toolbar control's availability at each state.
**Verdict:** available only at exactly one row selected; absent or disabled at zero or more than one.

---

## Context menu

### T8.10 — Right-click selects before opening the menu, without disturbing an existing selection
*Covers P8.4.a · Verifies Design §3.5*

Right-click a row with nothing selected; separately, with three other rows already selected, right-click a row inside that selection; separately, right-click a row outside it.
**Verdict:** first case selects the clicked row alone; second case leaves the three-row selection exactly as it was; third case replaces the selection with the newly clicked row alone. The menu opens on whatever selection results in each case.

### T8.11 — Save as… and Delete are available for any selection size
*Covers P8.4.b · Verifies FR-17, Design §3.5, §4.1*

Open the context menu with one row selected, then with three.
**Verdict:** both items appear in both cases; Delete is styled `caution`; choosing it opens the identical confirmation dialog the toolbar's own Delete button opens (same wording, same styling) — not a second dialog implementation.

### T8.12 — Show details appears only for a single selected row
*Covers P8.4.c · Verifies FR-28, Design §3.5, §8.9*

Open the context menu with one row selected, then with three.
**Verdict:** present in the first case, absent (not present-but-disabled) in the second.

### T8.13 — Preview appears only for one supported, in-cap row
*Covers P8.4.d · Verifies FR-30, Design §3.5, §8.10*

Open the context menu on: a single `.png` file within C-5's cap; a single `.exe` file; a single `.txt` file recorded over C-5's cap; three selected files including a supported one.
**Verdict:** Preview appears only in the first case; absent, not disabled, in the other three.

### T8.14 — Replace… appears only for a single selected row
*Covers P8.4.e · Verifies Design §3.5, §8.7*

Open the context menu with one row selected, then with three.
**Verdict:** present in the first case, absent in the second — mirroring T8.9's toolbar behaviour exactly.

### T8.15 — The menu never offers rename, move, or open-with
*Covers P8.4.f · Verifies Design §1.2, §3.5, HC-2*

Inspect the context menu's full item list across every selection state exercised above, and scan `main.ts`/`index.html` for any such affordance.
**Verdict:** none found anywhere.

---

## Details panel

### T8.16 — Details shows the right fields, no hash, and dismisses freely
*Covers P8.5.a, P8.5.b, P8.5.c, P8.5.d · Verifies FR-28, Design §8.9*

Add a file with a source modification time distinct from its added time; open Show details on it.
**Verdict:** name, folder, exact byte size, `Modified` (the source's own mtime, distinct from `Added`), and `Added` are shown, labelled the same words the list columns use plus `Modified`; no content hash appears anywhere; the panel dismisses via its close control, clicking outside it, or Escape, without blocking interaction with the rest of the screen while open.

---

## Preview overlay

### T8.17 — Preview opens as an overlay and returns to the exact prior state on close
*Covers P8.6.a, P8.6.b, P8.6.e · Verifies FR-30, Spec §5.3, Design §8.10*

Scroll the list partway, select a supported in-cap file well below the fold, open Preview, then close it.
**Verdict:** the overlay appears above the list (no new window, no navigation); its header names the file and offers a close control; after closing, the same row is selected and the list is scrolled to the same position as before opening.

### T8.18 — Image and text payloads render as Design §8.10 specifies
*Covers P8.6.c, P8.6.d · Verifies FR-30, Design §8.10*

Preview a small `.png`; separately, preview a `.md` file containing Markdown syntax (headings, a link).
**Verdict:** the image renders visibly, matching the source file; the Markdown file's raw text — including the syntax characters — is shown unrendered, in the body font (not monospace); no heading, link, or other Markdown construct is rendered as anything but plain text.

### T8.19 — Closing the preview revokes its object URL and releases its content
*Covers P8.7.a · Verifies FR-30, Spec §5.3*

Preview an image, note the object URL created for it (via devtools), then close the preview.
**Verdict:** the object URL is revoked (a subsequent load attempt against it fails) and no reference to the decoded payload remains reachable from module state afterward.

### T8.20 — Locking the vault clears an open preview and details panel
*Covers P8.7.b · Verifies FR-3, FR-30, Design §8.10*

With a preview open, lock the vault; separately, with the details panel open, lock the vault.
**Verdict:** in both cases, locking closes the overlay/panel and clears its content via the same routine T8.19 checks — not left open behind the locked screen, and not a second, parallel clearing path.

### T8.21 — Quitting the application runs the same clearing routine
*Covers P8.7.c · Verifies FR-30, Spec §5.3*

Inspect the `beforeunload` (or equivalent exit) handler's source.
**Verdict:** it calls the identical function P8.7.a defines and P8.7.b's lock path calls — not a second implementation written for this call site.

### T8.22 — No visible message announces preview-content clearing
*Covers P8.7.d · Verifies Design §7, §8.10*

Trigger each of close, lock, and exit with a preview open, watching the screen throughout.
**Verdict:** nothing appears stating that content was cleared, released, or forgotten — Design §8.10 treats this as expected behaviour, not an event worth surfacing.

### T8.23 — A failed preview integrity check reads identically to a failed extraction
*Covers P8.8.a · Verifies FR-18, FR-30, Design §6, §8.10*

Corrupt a supported, in-cap entry's stored ciphertext; attempt Preview on it; separately, attempt Save as… on the same corrupted entry.
**Verdict:** both surfaces show the exact same wording for the failure — the preview overlay does not carry a rewritten or preview-specific version of the message.

---

## Vocabulary and packaging

### T8.24 — New strings hold the fixed vocabulary and match the CLI's field labels
*Covers P8.9.a, P8.9.b · Verifies Design §3.4, §7*

Run `cargo test -p veil-gui` (extended `tests/vocabulary.rs` scan, which already walks all of `ui/src`) after this phase's UI strings exist; separately, compare the details panel's field labels against `veil detail`'s table headings (P7.1.b).
**Verdict:** the vocabulary scan reports no violation; the details panel's `name`/`folder`/`size`/`Modified`/`Added` labels match the CLI's own words for the same facts.

### T8.25 — Version and build succeed at 2.1.0
*Covers P8.10.a, P8.10.b · Verifies Requirements §8, Spec §8*

After the version bump, inspect the workspace `Cargo.toml`, `crates/veil-gui/ui/package.json`, and `crates/veil-gui/tauri.conf.json`; run `cargo build --workspace` and the frontend's `npm run build`.
**Verdict:** all three files read `2.1.0`; both builds succeed.

---

## Coverage

Preview's server-side guarantees — no ciphertext read on refusal, no disk touch, no disclosure — are Phase 7's closed record (T7.9 through T7.15) and are not re-checked here; this phase's cases assume `preview_entry` behaves as proved and check only that the browsing screen calls and clears it correctly. FR-24 (staleness) is not specifically exercised by any case in this document, per Phase8-ToDo.md's own coverage note — this phase's controls read the same snapshot the list already held, introducing no new staleness path.
