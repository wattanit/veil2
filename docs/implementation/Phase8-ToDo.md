# Veil2 — Phase 8 To-Do: Browsing Screen

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-12
**Owner:** wattanit
**Foundation and plan versions this list is built against:**
- Requirements Document v1.1 — upstream
- Design Guideline v1.4 — upstream
- Technical Specification v1.1 — upstream
- Implementation Plan v1.0 — upstream; this list expands Plan tasks P8.1–P8.10

This document owns the step-level breakdown of Phase 8. It defers what to build to the Requirements, how it presents to the Design Guideline, how it is built to the Specification, and phase sequencing to the Implementation Plan. Enumerated checks live in [Phase8-TestCases.md](Phase8-TestCases.md).

---

## Conventions

**Item identifiers** are `P8.<task>.<letter>`.

**Status** follows the Plan's convention: **built, carries forward** / **built, needs rewrite** / **built, remove entirely** / **not yet built**. Everything in this phase is **not yet built**.

**Done** for an item means the cited behaviour is observable — from `cargo tauri dev` for anything rendering or interactive, or from the node-shelled logic tests Phase 7's T7.4 established for anything that is pure frontend logic — and the test cases listed against it pass.

---

## What Phase 8 is for

Phase 7 proved `preview_entry`, `detail`'s CLI twin, and extension derivation hold their own guarantees, off a webview, before anything was built on top of them (Spec M7, Plan's own sequencing note). Phase 8 is that webview work: the browsing screen itself carries the second grouping dimension, sort, multi-select, the context menu, the details panel, and preview, per Design Guideline v1.4. Per Spec §5.3, almost none of this touches a Tauri command — grouping, sorting, and selection are frontend state over the list `list_entries` already returns in full; `preview_entry` is the one command this phase's UI calls, and it already exists. This phase's own new surface is therefore concentrated in `crates/veil-gui/ui/src/main.ts` (currently a plain TypeScript/DOM app, no framework, no client-side state library — confirmed by Spec §5.3's explicit statement that none is being added) and `index.html`/`styles.css`, plus one addition to `api.ts`'s `EntryInfo` interface and one new wrapper for `preview_entry`.

Nothing here changes the storage format, the cryptographic construction, or any Tauri command's guarantees — those are Phase 7's closed record. A defect found here that turns out to live in `preview_entry` or `detail` reopens Phase 7's record, not this one.

---

## P8.1 — Grouping control (none / folder / extension)

*Plan P8.1 · Spec §5.3 · FR-8, FR-29, Design §3.2*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P8.1.a | Done | `#group-toggle`'s boolean checkbox replaced with `#group-select`, a three-way `<select>` (none / folder / extension); module state `grouping: 'none' \| 'folder' \| 'extension'` replaces `grouped: boolean` in `main.ts` | FR-8, FR-29, Design §3.2 | T8.1 |
| P8.1.b | Done | `renderList()` gains an extension-grouping branch, wiring up P7.2.b's dormant `extensionOf` (`ui/src/extension.ts`) as its first caller anywhere in the frontend; the no-extension bucket is a reserved group, labelled `(no extension)` the same way the CLI's peer group is (T7.5) | FR-29, Design §3.2 | T8.1 |
| P8.1.c | Done | Per-group collapse/expand: a `Set<string>` of collapsed group keys in module state, toggled by clicking a `.group-header`; a collapsed header still renders its row count (`list.ts`'s `ListRow` group variant gained `key`/`count`/`collapsed`) | Design §3.2 | T8.2 |
| P8.1.d | Done | Changing the grouping choice (the `#group-select` change handler), or a lock/reopen cycle (`enterVault()`), clears the collapsed-set back to empty (every group expanded) | Design §3.2 | T8.3 |

---

## P8.2 — Column-header click-to-sort

*Plan P8.2 · Design §3.2*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P8.2.a | Done | The four column headers (`col-name`, `col-folder`, `col-size`, `col-added` in `index.html`) gained `data-column` and a nested `.sort-arrow` span; clickable via a listener on `#list-header`, styled with a pointer cursor | Design §3.2 | T8.5 |
| P8.2.b | Done | Sort state `sortColumn: SortColumn \| null` / `sortDirection` in `main.ts`; first click on a header sorts ascending by it, a second click on the same header reverses to descending, a click on a different header resets to ascending on the new one; `updateSortArrows()` reflects the active column/direction | Design §3.2 | T8.4, T8.5 |
| P8.2.c | Done | Comparator logic extracted as pure functions in a new module, `ui/src/sort.ts` (case-insensitive `localeCompare` for name/folder, numeric subtraction for size/added), tested the way `extension.ts` is (`crates/veil-gui/tests/sort.rs`) rather than only by driving the DOM. `renderList()` applies `applySort()` to the flat list *before* bucketing into groups, so each group's bucket inherits the already-sorted order — no second per-group sort pass needed. **One correction found while testing:** descending negates the ascending comparator rather than reversing the ascending-sorted array; for a stable sort these agree only when no two rows tie — a tied pair keeps its own relative order in both directions. T8.4's fixture and Phase8-TestCases.md's verdict were corrected to describe this rather than the (wrong) "exact reverse including ties" claim the test first asserted and failed | Design §3.2 | T8.4, T8.6 |
| P8.2.d | Done | No separate sort control exists in the controls bar — sort is reached only by clicking a header, per Design §3.2 | Design §3.2 | T8.5 |

---

## P8.3 — Multi-select

*Plan P8.3 · Design §3.2, §3.5*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P8.3.a | Done | `selectedId: number \| null` replaced by `selectedIds: Set<number>` plus `lastClickedId` (the shift-range anchor); pure selection-transition logic extracted into a new module, `ui/src/selection.ts`, given the current visually-ordered id list (`EntryList.entryIds()`, new) and a click kind (`plain` / `shift` / `cmd`) | Design §3.2 | T8.7 |
| P8.3.b | Done | Wired into `setupSelection()`: a plain click selects one row and clears the rest; shift-click extends a contiguous range from the anchor to the clicked row, in the current sorted/grouped visual order; Cmd-click toggles one row into or out of the selection without disturbing the rest, and becomes the new anchor. **Found and fixed along the way:** `EntryList` never tracked selection itself — the "selected" class was only ever a one-off DOM mutation from the click handler, which the very next scroll-triggered `renderVisible()` silently erased by resetting `className` wholesale. Gave `EntryList` its own `setSelection()` and had `renderRow` apply it on every render, the same way it already applies `unreadable` — latent since Phase 6's single-select, but multi-select made it worth fixing rather than working around | Design §3.2 | T8.7 |
| P8.3.c | Done | Delete acts on the full selection via a new `selectedEntries()` helper — `runDelete()` already looped an array (Phase 6), so this is the UI enabling it for more than one row, not a new deletion path | Design §3.2, §4.1 | T8.8 |
| P8.3.d | Done | Replace… stays available only when exactly one row is selected (`updateSelectionButtons()`) — absent or disabled otherwise, since it has no meaning for more than one file | Design §3.2, §8.7 | T8.9 |

---

## P8.4 — Right-click context menu

*Plan P8.4 · Design §3.5*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P8.4.a | Done | `setupContextMenu()`'s `contextmenu` listener on `#list-spacer`: right-clicking a row not already part of the current selection replaces the selection with that row alone before the menu opens; right-clicking a row already inside a multi-selection opens the menu on the existing selection unchanged (platform convention) | Design §3.5 | T8.10 |
| P8.4.b | Done | **Save as…** (FR-17) and **Delete** appear for any selection size; Delete is styled `caution` and reuses the exact confirmation dialog the toolbar's Delete button already opens — no second dialog. **Scope decision, recorded in Design Guideline v1.4:** a multi-row Save as… runs the existing single-file `extract()` save dialog once per file in sequence rather than a new destination-folder picker — found while implementing that Design §3.5 credited "the toolbar" with an extraction control that has never existed (only the double-click has), and that a folder-picker would need its own overwrite-collision check with no existing mechanism to build it from. Corrected the Design Guideline's wording and opened a new §9 item on the folder-picker question rather than building it unasked | FR-17, Design §3.5, §4.1 | T8.11 |
| P8.4.c | Done | **Show details** (FR-28) appears only when exactly one row is selected — a multi-row selection has no single set of metadata to show, so the item is absent, not disabled. Its click handler is a stub (`showDetails()`) until P8.5 builds the real panel | FR-28, Design §3.5, §8.9 | T8.12 |
| P8.4.d | Done | **Preview** (FR-30) appears only when exactly one row is selected and it passes the new `isPreviewable()` (`ui/src/previewable.ts`) — the same eleven-extension list and C-5 cap `preview.rs`'s `classify` and size check use, a fourth deliberately-separate implementation of the lookup rather than a call to `extensionOf`, for the same reason `classify` gives; absent, not disabled, for an unsupported type, an over-cap entry, or a multi-row selection. Its click handler is a stub (`openPreview()`) until P8.6 builds the real overlay | FR-30, Design §3.5, §8.10 | T8.13 |
| P8.4.e | Done | **Replace…** appears only when exactly one row is selected, mirroring the toolbar button (P8.3.d) | Design §3.5, §8.7 | T8.14 |
| P8.4.f | Done | The menu offers no rename, move-to-folder, or open-with-external-application item — confirmed by the five-item list `openContextMenu()` builds and nothing else | Design §3.5, §1.2, HC-2 | T8.15 |

---

## P8.5 — Details panel

*Plan P8.5 · Design §8.9 · FR-28*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P8.5.a | Not yet built | `api.ts`'s `EntryInfo` interface gains `sourceMtime: number` — the Rust struct (`commands.rs`) has carried this since Phase 7 (P7.1.a); the frontend type never picked it up, since nothing consumed it until now | FR-28, Spec §5.1 | T8.16 |
| P8.5.b | Not yet built | A details panel/popover, opened from **Show details** (P8.4.c): name, folder, exact byte size, `Modified` (from `sourceMtime`), `Added` — labelled the same words the list columns use, plus `Modified`, which the list has no room for | FR-28, Design §8.9 | T8.16 |
| P8.5.c | Not yet built | No content hash shown — the same decision Phase 7's `detail` CLI output already made (P7.1.d), held here too so the two peers agree | FR-28, Design §8.9 | T8.16 |
| P8.5.d | Not yet built | Dismissible freely (a close control, click-outside, or Escape) — not a modal blocking the rest of the screen | Design §8.9 | T8.16 |

---

## P8.6 — Preview overlay

*Plan P8.6 · Design §8.10 · FR-30, Spec §5.3*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P8.6.a | Not yet built | `api.ts` gains `previewEntry(id)`, a typed wrapper over `invoke('preview_entry', ...)` returning the `PreviewPayload` union (`{ kind: 'image', mime, base64 }` / `{ kind: 'text', content }`, matching `preview.rs`'s `#[serde(tag = "kind")]` shape) | FR-30, Spec §5.3 | T8.17 |
| P8.6.b | Not yet built | Preview opens as an overlay above the list (not a separate window, not a route change); its header names the file and carries a close control | Design §8.10 | T8.17 |
| P8.6.c | Not yet built | An `Image` payload is rendered by building an object URL from the base64 (`data:` URL or a `Blob` + `URL.createObjectURL`) | FR-30, Spec §5.3 | T8.18 |
| P8.6.d | Not yet built | A `Text` payload — including `.md`, per FR-30's decision not to render Markdown — is shown unrendered, in the body font, not monospace | FR-30, Design §8.10 | T8.18 |
| P8.6.e | Not yet built | Closing the overlay returns to exactly the selection and scroll position it was opened from | Design §8.10 | T8.17 |

---

## P8.7 — Clearing preview and details state

*Plan P8.7 · Design §8.10 · FR-3, FR-30, Spec §5.3*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P8.7.a | Not yet built | One clearing routine, called from every path that ends a preview: closing the overlay revokes any object URL created for it (P8.6.c) and drops the held payload reference | FR-30, Spec §5.3 | T8.19 |
| P8.7.b | Not yet built | `lock()`'s existing clearing sequence (already clears `allEntries`, `selectedIds`, etc.) extended to also call P8.7.a's routine and dismiss the details panel if open — the same function reference, not a parallel one written for this call site | FR-3, FR-30, Design §8.10 | T8.20 |
| P8.7.c | Not yet built | The same routine runs on application exit (`beforeunload`), rather than a second implementation of "clear preview state" written for that event | FR-30, Spec §5.3 | T8.21 |
| P8.7.d | Not yet built | None of P8.7.a–c surfaces a visible message — Design §8.10 is explicit that this is not an event worth announcing, only a defect if it fails to happen | Design §8.10, §7 | T8.22 |

---

## P8.8 — Preview integrity-failure wording parity

*Plan P8.8 · Design §6, §8.10 · FR-18, FR-30*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P8.8.a | Not yet built | A failed integrity check surfaced through `preview_entry` (the same `Corrupt`-family error `extract` already reports) is shown inside the preview overlay with the identical wording the existing extraction-failure path uses elsewhere in this screen — reusing that string/formatting function, not a preview-specific rewrite of it | FR-18, FR-30, Design §6, §8.10 | T8.23 |

---

## P8.9 — Vocabulary audit

*Plan P8.9 · Design §7*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P8.9.a | Not yet built | Every new user-facing string this phase introduces — menu item labels, details-panel labels, preview chrome, the grouping control's three option labels — checked against `tests/vocabulary.rs`'s existing fixed-vocabulary scan of `ui/src`. The scanner already walks the whole tree (Phase 7 extended it to `veil-gui/src` too), so this item is confirming new strings pass it, not extending what it scans, unless a violation surfaces | Design §7 | T8.24 |
| P8.9.b | Not yet built | Details panel's field labels (`name`, `folder`, `size`, `Modified`, `Added`) checked against `detail`'s CLI output (P7.1.b) for the same words, since Design §3.4 asks the two peers not to drift | Design §3.4, §8.9 | T8.24 |

---

## P8.10 — Version and packaging

*Plan P8.10 · Requirements §8, Spec §8*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P8.10.a | Not yet built | Workspace `Cargo.toml` (`version` and the `veil-core` pin), `crates/veil-gui/ui/package.json`, and `crates/veil-gui/tauri.conf.json` bumped together, `2.0.1` → `2.1.0` — the three cannot drift, per the workspace manifest's own comment | Requirements §8, Spec §8 | T8.25 |
| P8.10.b | Not yet built | `cargo build --workspace` and the frontend's `npm run build` both succeed post-bump; no tag or push performed as part of this item — that is a separate, explicit request | Spec §8 | T8.25 |

---

## Coverage note

**Rendering and interaction items (most of P8.1 through P8.8) are checked live in `cargo tauri dev`, by a person looking at it** — the same convention Phase 6's TestCases document established (its Conventions §, kind 2) for exactly this reason: this project has no browser/DOM automation harness, and none is being added for one phase's worth of UI wiring. Where a piece of logic can be pulled out as a pure function instead (sort comparators, selection-set transitions), it is, and gets the same node-shelled automated check Phase 7's T7.4 established for `extension.ts` — not because the DOM checks are optional, but because automating what can be automated narrows what a person has to look at by hand.

**Preview's server-side guarantees (no disk touch, no ciphertext read on refusal, no disclosure) are Phase 7's closed record (T7.9–T7.15) and are not re-proved here.** Phase 8 only has to prove it *calls* `preview_entry` correctly and clears what it receives — not that `preview_entry` itself behaves, which would be redoing Phase 7's work.

**FR-24 (staleness) is not specifically exercised by this phase's new controls.** Grouping, sorting, selection, and preview all read from the same `allEntries` snapshot the list already held before this phase; none of them re-reads the vault directory independently, so none of them introduces a new staleness path beyond what Phase 6 already covers.

---

## Exit

The grouping control offers none/folder/extension and groups collapse and expand per the session; every column sorts ascending then descending on repeated clicks and composes correctly with grouping; multi-select follows platform convention for click/shift-click/Cmd-click and every selection-wide action (Delete, Save as…) honours the full selection; the context menu offers exactly the five items Design §3.5 lists, each present under exactly the condition it specifies, and nothing else; the details panel and preview overlay show what Design §8.9/§8.10 specify and nothing more; preview and details content is cleared on close, lock, and exit with no visible announcement of the fact; a failed preview integrity check reads identically to a failed extraction; the vocabulary audit is clean; and the release is tagged 2.1.0.

---

## Open Questions

- **Whether P7.5's still-open "base64 payload size near C-5's cap" question gets resolved here.** Phase 8 is the first place a frontend actually receives a `preview_entry` payload over IPC, so a real near-cap image fixture, exercised through this phase's overlay, is the natural place to finally measure it — carried over from Phase 7's own open item rather than restated as new.
- **Preview's keyboard shortcut and a collapsed group's exact visual treatment**, per Design Guideline §9, remain open past this phase's exit — neither blocks it, per the Plan's own sequencing note.
