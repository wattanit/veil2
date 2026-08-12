# Veil2 — Phase 5 To-Do: GUI Foundation

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-10
**Owner:** wattanit
**Foundation and plan versions this list is built against:**
- Requirements Document v1.0 — upstream
- Design Guideline v1.0 — upstream
- Technical Specification v1.0 — upstream
- Implementation Plan v1.0 — upstream; this list expands Plan tasks P5.1–P5.6

This document owns the step-level breakdown of Phase 5. It defers what to build to the Requirements, how it presents to the Design Guideline, how it is built to the Specification, and phase sequencing to the Implementation Plan. Enumerated checks live in [Phase5-TestCases.md](Phase5-TestCases.md).

**Nothing in Phase 5 existed before this phase.** `crates/veil-gui` was a placeholder binary depending on `veil-core` and nothing else — no Tauri, no frontend, no packaging. Every item below is new build, not carried forward.

---

## Conventions

**Item identifiers** are `P<phase>.<task>.<letter>`.

**Status** follows the Plan's convention: **built, carries forward** / **built, needs rewrite** / **built, needs review** / **not yet built**.

**Done** for an item means the cited behaviour is observable, the test cases listed against it pass, and the Plan's definition of done holds.

---

## What Phase 5 is for

Phase 4 proved `veil-core` survives interruption. Phase 5 proves the one thing the GUI exists to do at all — render a vault's own filenames correctly, in a shell that does not leak them to disk — before any feature is built on top of that shell. Design Guideline §1.1: the file list is the primary interface. If it cannot render the files, nothing built on it in Phase 6 matters.

**Frontend stack decision.** The Specification fixes Tauri v2 over a system webview (§5.3) but does not name a frontend framework. This phase builds the webview content in vanilla TypeScript, HTML, and CSS — no framework. Reasoning: §7's dependency policy already treats the JavaScript toolchain Tauri brings in as an accepted cost to be minimized, not multiplied; a single-panel list with no client-side routing and no complex shared state (Design §3.1, §1.2 — this is not a file manager) does not need a framework's state-management machinery; and every additional pinned frontend dependency is one more thing §7's audit gates run against. If the virtualised list or the theming in Phase 6 later strains vanilla DOM manipulation, that is a decision to revisit then, against a concrete cost — not a default to reach for now.

**Scope boundary with Phase 6.** Phase 5 is the shell and the list, not the product. There is no unlock screen (P6.1), no vault-creation flow (P6.3), no identity bar wired to real lock state (P6.4) — those are Phase 6's. What this phase needs to prove P5.4 and P5.5 is a fixture: a harness command that opens a vault already prepared with fixture entries (including the complex-script names P5.5 requires) using a fixed test password, bypassing the unlock UI Phase 6 will build. This is a test fixture, not a feature, and it does not ship — `#[cfg(debug_assertions)]` or an equivalent gate keeps it out of release builds (mirroring how `KdfParams::for_tests()` is kept out of `veil-core`'s release builds, P1.1.d).

---

## P5.1 — Tauri shell over `veil-core`

*Plan P5.1 · Spec §5.3 · A-3, A-4*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P5.1.a | Done | Tauri v2 added to `veil-gui`: `Cargo.toml` dependencies, `tauri.conf.json`, and a `ui/` directory of vanilla HTML/CSS/TS as the bundled frontend | Spec §5.3, §7 | T5.1 |
| P5.1.b | Done | One Tauri command per `veil-core` operation the GUI needs in this phase: open, list entries, extract, close. Each takes the same arguments and returns the same facts as the CLI's equivalent (A-4) | A-4, Spec §5.1 | T5.1, T5.2 |
| P5.1.c | Done | Every command runs the vault operation on a worker thread (`tauri::async_runtime::spawn_blocking`) — never on the thread that services the webview — so a large extract cannot make the interface unresponsive | A-3, Spec §5.3 | T5.3 |
| P5.1.d | Done | Progress reaches the UI thread through Tauri's event channel, not through the command's return value, so a long operation's progress is visible before it finishes | A-3, Spec §5.3 | T5.3 |
| P5.1.e | Done | A cancel command that reaches the same `Cancel` token the running operation holds, in a lock separate from the vault's own so it is never queued behind the operation it interrupts | A-3, FR-15, FR-20 | T5.3 |

**Not built in this phase, deliberately:** `add`, `replace`, and `delete` commands. P5.6 needs `add` to prove the drop target end-to-end and is where it is built; `replace` and `delete` have no UI to drive them before Phase 6 and stay unbuilt until then rather than being wired ahead of anything that calls them.

---

## P5.2 — Ephemeral webview storage

*Plan P5.2 · Spec §5.3 · HC-1*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P5.2.a | Done | The main window built in `lib.rs`'s `setup` hook (not `tauri.conf.json`'s declarative `app.windows`, which has no field for this) with `.incognito(true)` — on macOS this selects `WKWebView`'s `nonPersistentDataStore` | Spec §5.3, HC-1 | T5.4 |
| P5.2.b | Done | Confirmed once, by hand: browsed the fixture vault, quit, and inspected `~/Library/WebKit/veil-gui/` for the fixture's Thai, Arabic, Han, and emoji names, in every file, as raw UTF-8 bytes | Spec §5.3, HC-1 | T5.4 |

Spec §5.3 is explicit that this item carries no dedicated test suite and no per-platform release gate — the worst a lapse leaks is filenames, not content. T5.4 is the one-time confirmation the Specification asks for, not a regression suite.

**What P5.2.b actually found, and why it is still a pass.** `incognito(true)` governs the *page's* storage — cookies, `localStorage`, IndexedDB, HTTP cache — and that part is genuinely non-persistent. It does not stop WebKit's own system-level Resource Load Statistics and Private Click Measurement databases (`pcm.db`, `observations.db`) from being created and written under `~/Library/WebKit/veil-gui/WebsiteData/`, regardless of the data-store setting — these are OS/WebKit privacy-telemetry bookkeeping about cross-site tracking classification, not page content, and no API this app has reaches them. Confirmed by direct inspection that none of the fixture's four names appear in any of those files. What Spec §5.3 and HC-1 actually forbid is a name leaking; a WebKit-internal table that never had the opportunity to see one (this app's CSP admits no cross-origin request for it to classify) is not that, and "nothing at all touches disk" was never the claim on offer.

---

## P5.3 — CSP, storage APIs, and devtools

*Plan P5.3 · Spec §5.3 · HC-1*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P5.3.a | Done | `tauri.conf.json`'s Content-Security-Policy restricted to the bundled origin and Tauri's own IPC channel — no remote origin can be loaded or fetched from | Spec §5.3, §7 | T5.5 |
| P5.3.b | Done | The frontend source contains no reference to `localStorage`, `sessionStorage`, or `indexedDB` | Spec §5.3, HC-1 | T5.6 |
| P5.3.c | Done | DevTools reachable only from an explicit `--features devtools` invocation (`cargo tauri dev --features devtools`), never from `cargo tauri build`. `cfg(debug_assertions)` cannot gate this in `Cargo.toml` — confirmed by running the build: Cargo warns and ignores it, since that table only understands platform predicates, not profile ones | Spec §5.3 | T5.7 |

---

## P5.4 — The entry list

*Plan P5.4 · Design §2.3, §3.2*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P5.4.a | Done | Columns **name, folder, size, added**, in that order — the same order as the CLI's table (Design §3.2, §3.4, A-4) | Design §3.2 | T5.8 |
| P5.4.b | Done | List rows `28px`; body and row text `13px`, metadata `11px`, headings `15px` semibold; System UI font throughout, no monospace | Design §2.3 | T5.8 |
| P5.4.c | Done | Size and date columns rendered with tabular figures (`font-variant-numeric: tabular-nums`) — fixed regardless of the other tunable values | Design §2.3 | T5.9 |
| P5.4.d | Done | The list is windowed: rows are pooled and only those near the viewport exist in the DOM (`renderVisible` in `main.ts`), so a vault of several thousand entries does not render several thousand rows at once | Design §2.3 | T5.10 |

---

## P5.5 — Complex-script rendering

*Plan P5.5 · Design §2.2*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P5.5.a | Done | A fixture vault with Thai, Arabic, Han, and emoji entry names (`fixture.rs`), `cfg(debug_assertions)`-gated the same way `KdfParams::for_tests()` is in `veil-core` | Design §2.2 | T5.11 |
| P5.5.b | Done | Each fixture name confirmed to render as the correct glyphs, not tofu or a mojibake substitution, in both the light and dark palettes of Design §2.2 — confirmed live in `cargo tauri dev`, by the person who would actually notice a wrong glyph | Design §2.2 | T5.11 |

This is the evidence Spec §5.3 says decided the toolkit over egui. It is checked visually — no automated shaping oracle exists — and is recorded once per Design Guideline change that touches the affected tokens, not on every commit.

---

## P5.6 — Drop target and native dialogs

*Plan P5.6 · Design §3.3 · FR-9, FR-17*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P5.6.a | Done | With a vault open, the whole window is a drop target (`getCurrentWebview().onDragDropEvent`); on drag-enter, an affordance states the count before release — confirmed live | Design §3.3 | T5.12 |
| P5.6.b | Done | Releasing the drop calls `add_files` once per dropped path, leaving the source files untouched (FR-9) — confirmed live | Design §3.3, FR-9 | T5.13 |
| P5.6.c | Deferred to Phase 6 | With no vault open, dropping a vault directory opening it needs a password, and there is no user-facing way to supply one yet — the fixture bypass is the only "open" this phase has. The half that *is* buildable now (a drop refused with an explanation when it can't be added) already happens through `add_files`'s ordinary error path; the vault-opening half waits for Phase 6's unlock screen | Design §3.3 | T5.14 (not run) |
| P5.6.d | Done | A native save dialog (`tauri_plugin_dialog`, called from Rust so the frontend needs no plugin-specific JS) for the extraction destination, triggered by double-clicking a row — confirmed live | Design §3.3, FR-17 | T5.15 |

---

## Exit

- Thai, Arabic, Han, and emoji filenames render correctly in a fixture vault, in both light and dark (P5.5). **Met** — confirmed live.
- Dropping 34 files shows "34" before release (P5.6.a). **Met** — confirmed live (the drag-enter affordance names the count).
- The webview is configured for ephemeral storage, and this has been confirmed once by hand (P5.2). **Met**, with the WebKit-telemetry nuance recorded under P5.2.b.
- No frontend dependency can reach a network origin outside the bundle; no persistent browser storage API is referenced by the frontend source; DevTools do not ship in a release build (P5.3). **Met**, all three now enforced by `tests/structure.rs` (T5.5–T5.7) rather than resting on a one-time manual check.

**Everything above is built, and `cargo test -p veil-gui` exercises what can be exercised without a real webview:** T5.2 and T5.3 drive the command layer through `tauri::test`'s mock runtime (open, list, extract, cancel, progress events — including a genuine 16 MiB cancel-in-flight, not a structural assertion that never actually raced); T5.5–T5.7 check the CSP, the frontend source, and the resolved dependency graph mechanically. What a mock runtime cannot exercise — real window creation, real drag-and-drop, real script rendering, the native save dialog — was checked live in `cargo tauri dev` instead, once, by a person looking at it, which is what Phase5-TestCases.md's own conventions section says these cases are for.

**One item is deliberately not built: P5.6.c**, opening a vault by dropping it with none open. That needs a password prompt, and Phase 5 built no user-facing way to supply one — the Plan's own Phase 5 Exit criterion does not require it either. Waits for Phase 6's unlock screen.

**`cargo check --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --check` are all clean**, `veil-gui` included. `npm run build` under `crates/veil-gui/ui` type-checks against the real `@tauri-apps/api` type declarations and bundles with `esbuild`; `npm audit` is clean — one runtime dependency (`@tauri-apps/api`, first-party Tauri), two dev-only build tools (`esbuild`, `typescript`), nothing else in the tree.

**`cargo deny check` needed real work, not a rubber stamp, and `deny.toml` now carries it.** Adding Tauri's dependency tree tripped every category the file enforces: `anyhow` reached the graph through Tauri's own internal crates (not `veil-gui`'s code, which uses none), ~25 crates needed a version Tauri's tree pins differently than `veil-core`'s does, two new licences (`MPL-2.0`, `Zlib`) showed up with no MIT/Apache-2.0 alternative, and 16 advisories fired for Linux-only GTK3 bindings and build-time proc-macro/codegen dependencies reachable only through `[graph].targets`' Windows/Linux entries — nothing on the path a macOS build actually compiles. Checked each category rather than widening a threshold: the `anyhow` wrappers list now names Tauri's own crates as the direct parents they are; a `skip-tree` rooted at Tauri's three entry points from `veil-gui` (pinned to exact versions) resolves the version duplicates without touching `veil-core`'s own single-version cryptographic dependencies, confirmed unaffected by tracing `sha2`/`digest`/`crypto-common`'s two version lineages separately; the licences are genuinely permissive, just not previously seen; the advisories are recorded with the reason each is unreachable. One stale, no-longer-needed `windows-sys` skip from before Tauri existed was removed along the way — cargo-deny said `unnecessary-skip`, and it was right. `cargo audit` (a different tool, not `deny.toml`) still prints these as informational warnings and exits 0 either way; same set, one additional GTK/glib advisory its whole-lockfile scan sees that `cargo deny`'s per-target graph does not, same reason it doesn't matter.

---

## Open Questions

- **Application icon and installer identity.** Design §9 notes this is not yet designed, and its anti-goals (§1.2) rule out padlocks and shields — most of the conventional category. Not gating for Phase 5, which needs no installer; carried to Phase 6 or later, whichever actually packages a release. Resolver: owner.
- **Whether the vanilla-TypeScript frontend decision holds once Phase 6's fuller interaction surface (search, grouping, the unlock screen's states) is built against it.** This phase's list and drop target are simple enough that no framework's absence was felt; Phase 6 is a fair re-test of that. Resolver: owner, at Phase 6 exit.
