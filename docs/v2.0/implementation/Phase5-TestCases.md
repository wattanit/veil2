# Veil2 — Phase 5 Test Cases: GUI Foundation

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-10
**Owner:** wattanit
**Foundation and plan versions these cases are built against:**
- Requirements Document v1.0 — upstream
- Design Guideline v1.0 — upstream
- Technical Specification v1.0 — upstream
- Implementation Plan v1.0 — upstream
- [Phase5-ToDo.md](Phase5-ToDo.md) v1.0 — companion; each case names the item it covers

This document owns the enumerated checks that close Phase 5. Every case cites the requirement it verifies.

---

## Conventions

**Case identifiers** are `T<phase>.<n>`, sequential within this document.

**Two kinds of case, stated as such rather than blurred.** A Tauri command's logic — thread placement, progress events, argument shape — is ordinary Rust and is tested the same way `veil-core`'s API is: by calling it directly, using Tauri's own test harness (`tauri::test::mock_builder`) to stand in for a real window. Layout, colour, script rendering, and drag-and-drop are properties of a real webview compositing real pixels; no automated shaping or layout oracle is set up for this phase, so those cases are checked by hand, against the fixture of P5.5.a, and the case states exactly what was looked at rather than claiming a machine confirmed it.

**Where these run.** The development machine, macOS. Windows and Linux are not in scope (Requirements §2.1) and are not run.

---

## The shell and its commands

### T5.1 — The shell launches and the bundled frontend loads
*Covers P5.1.a · Verifies Spec §5.3*

Build and launch the Tauri application; confirm the window opens and the bundled `ui/` content is what renders, with no console error naming a missing asset or a blocked resource.
**Verdict:** the window opens; the page loads from the bundle, not a remote origin.

### T5.2 — Every GUI command mirrors its CLI equivalent
*Covers P5.1.b · Verifies A-4*

For each command built this phase (open, list entries, extract, close), call it directly through Tauri's mock harness and compare its arguments and returned facts against the CLI's equivalent for the same vault and operation.
**Verdict:** same inputs accepted, same facts returned — an entry's name, folder, size, and id read identically through both surfaces.

### T5.3 — A running operation does not block, reports progress, and can be cancelled
*Covers P5.1.c, P5.1.d, P5.1.e · Verifies A-3, FR-15, FR-20*

Start an extract large enough to be genuinely in flight through the command layer. While it runs, confirm the command's own future yields (the worker thread, not the command dispatch thread, is doing the work), that progress events arrive on the event channel before completion, and that invoking the cancel command stops it and the operation reports cancelled.
**Verdict:** progress events observed before completion; cancellation takes effect; the calling side was never blocked waiting for the operation to finish before it could do anything else.

---

## Webview security configuration

### T5.4 — The webview leaves no persistent trace of a vault's names
*Covers P5.2.a, P5.2.b · Verifies HC-1, Spec §5.3*

Confirm the webview configuration requests non-persistent storage. Then, by hand: open the fixture vault, browse it, quit, and inspect `~/Library/WebKit/veil-gui/` for the fixture's Thai, Arabic, Han, and emoji names, as raw UTF-8 bytes, in every file underneath it.
**Verdict:** the configuration is non-persistent; no fixture name is found on disk afterward. Some files are found regardless — `WebsiteData/ResourceLoadStatistics/{pcm,observations}.db` — which is WebKit's own cross-site-tracking bookkeeping and outside what `incognito` or this app's API touches; the case is about names, and none reached those files either. Recorded once, per Spec §5.3's own framing — not a case that runs with the regression suite.

### T5.5 — The Content-Security-Policy admits no remote origin
*Covers P5.3.a · Verifies Spec §5.3, §7*

Inspect `tauri.conf.json`'s CSP directive.
**Verdict:** every source directive is `'self'` or a data URI the bundle itself supplies; no remote host appears anywhere in the policy.

### T5.6 — The frontend source references no persistent storage API
*Covers P5.3.b · Verifies HC-1, Spec §5.3*

Grep the `ui/` source tree for `localStorage`, `sessionStorage`, and `indexedDB`, the same denylist-over-source-tree approach T0.1 and T0.2 use for `veil-core`'s dependency graph.
**Verdict:** none present. A later addition fails this case, which is the point — a mechanical check outlives whoever wrote the frontend code that day.

### T5.7 — DevTools do not reach a release build
*Covers P5.3.c · Verifies Spec §5.3*

Confirm with `cargo tree -p veil-gui -e features -i tauri` that `tauri`'s `devtools` feature is absent by default and present only when built with `--features devtools`. Run `cargo tauri build` (what a release actually runs) and confirm no devtools entry point (menu item, keyboard shortcut, or right-click inspector) reaches the bundled binary.
**Verdict:** the feature, and the entry point it gates, are both absent from a plain `cargo tauri build`; both are present when `--features devtools` is passed, so the case is proof the gate is a real branch and not simply unused code. This is a Cargo feature flag on explicit invocation, not a `cfg(debug_assertions)` branch — `[target.'cfg(...)']` dependency tables only understand platform predicates, and Cargo warns and silently ignores a profile predicate placed there, which is how the first attempt at this item was found to not actually work.

---

## The entry list

### T5.8 — Columns, order, and density match the Design Guideline
*Covers P5.4.a, P5.4.b · Verifies Design §2.3, §3.2*

Render the fixture vault's list and inspect the rendered DOM: column headers and their order, computed row height, computed font sizes for row text, metadata text, and headings, and the computed font family.
**Verdict:** columns read name, folder, size, added, in that order; row height is 28px; body and row text compute to 13px, metadata to 11px, headings to 15px semibold; the computed font family is the system UI font, never a monospace fallback.

### T5.9 — Size and date figures align on tabular numerals
*Covers P5.4.c · Verifies Design §2.3*

Inspect the computed `font-variant-numeric` (or platform equivalent) on the size and added columns.
**Verdict:** tabular figures are in effect on both columns, independent of whatever the other tunable typography values are set to.

### T5.10 — The list is virtualised
*Covers P5.4.d · Verifies Design §2.3*

Open a fixture vault with several thousand entries and count the row elements actually present in the DOM against the number visible in the viewport.
**Verdict:** the DOM holds a bounded number of rows near the viewport, not one element per entry — scrolling changes which entries are present, not how many.

---

## Complex-script rendering

### T5.11 — Complex-script names render correctly in both themes
*Covers P5.5.a, P5.5.b · Verifies Design §2.2*

Open the fixture vault of P5.5.a, holding entries named in Thai, Arabic, Han, and with an emoji. Look at the rendered list in the light palette, then switch to dark and look again.
**Verdict:** every fixture name renders as its correct glyphs — no tofu (missing-glyph boxes), no mojibake, and Arabic's right-to-left shaping is visibly correct — in both palettes. This is the evidence Spec §5.3 cites as having decided Tauri over egui; a failure here calls that decision into question, not just this build.

---

## Drop target and dialogs

### T5.12 — Drag-enter names the count and destination before release
*Covers P5.6.a · Verifies Design §3.3*

With a vault open, drag a selection of files over the window without releasing.
**Verdict:** an affordance appears stating the file count and the destination vault before the drop completes.

### T5.13 — A drop adds every file and leaves the originals untouched
*Covers P5.6.b · Verifies Design §3.3, FR-9*

Drop several files onto an open vault's window.
**Verdict:** every dropped file appears in the list afterward; each source file is unmodified and still present at its original path.

### T5.14 — A drop with no vault open opens a vault or is refused
*Covers P5.6.c · Verifies Design §3.3* — **not run this phase**

With no vault open, drop a vault directory; separately, drop an ordinary file.
**Verdict:** the vault directory opens. The ordinary file is refused with a stated explanation, not silently ignored and not treated as a crash.

**Deferred to Phase 6.** Opening a dropped vault needs a password, and Phase 5 built no way to supply one — the fixture bypass (P5.5.a) is the only "open" this phase has, and it takes no password because it's a fixture. This case waits for Phase 6's unlock screen to have anything to drive.

### T5.15 — The extraction destination is chosen through a native dialog
*Covers P5.6.d · Verifies Design §3.3, FR-17*

Trigger an extraction from the list.
**Verdict:** the platform's native save dialog appears; the file is written to the chosen destination.

---

## Coverage

| Identifier | Case |
|---|---|
| HC-1 | T5.4, T5.6 |
| A-3 | T5.3 |
| A-4 | T5.2 |
| FR-9 | T5.13 |
| FR-15, FR-20 | T5.3 |
| FR-17 | T5.15 |

**Not reachable in Phase 5**, deferred to Phase 6: the unlock screen and its states, vault creation, the identity bar's real lock state, search, folder grouping, opening a vault by dropping it with none open (T5.14 — needs the unlock screen's password prompt), and every functional requirement whose only interface is one of those.

---

## Not covered, and why

**No automated shaping or layout oracle.** T5.8 through T5.11 are read by a person looking at rendered output because nothing in this build stands up a rendering-comparison pipeline; a screenshot-diff harness was considered and declined for this phase as disproportionate to a foundation phase whose entire remaining lifetime is Phase 6 rebuilding the surface around it. Revisit if Phase 6's larger surface makes hand-verification the bottleneck it currently is not.

**No WebDriver end-to-end suite.** T5.1, T5.12 through T5.15 exercise real window and OS-level drag-and-drop and dialog behaviour, which `tauri::test`'s mock harness does not model. `tauri-driver` would close this gap; declined for the same proportionality reason as the shaping oracle, and for the same reason `cargo-fuzz` was declined in Phase 1 (Spec §11) — a tool this development machine does not carry, weighed against a lower-depth manual check that runs today.
