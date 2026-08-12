# Veil2 — Implementation Plan

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-12
**Owner:** wattanit
**Foundation versions this plan is built against:**
- Requirements Document v1.1 — upstream
- Design Guideline v1.3 — upstream
- Technical Specification v1.1 — upstream

This document owns the **sequencing** of the work: ordered phases expanding the Technical Specification's M7 (Spec §10), each with entry and exit conditions, and each task citing the foundation item that put it there. It defers what to build to the Requirements, how it presents to the Design Guideline, and how it is built to the Specification. No task below restates a format, an algorithm, or a layout — each cites the Spec section that defines it. If implementation finds the Spec wrong or underspecified, that is a Specification version bump, not a correction recorded here.

**This plan is additive, not a rewrite.** Unlike the previous Implementation Plan's relationship to what came before it, nothing here changes the storage format, the cryptographic construction, or anything shipped in 2.0.0. The 2.0.0 record — how that release was actually sequenced and verified — stays exactly as it was, archived at `docs/v2.0/IMPLEMENTATION_PLAN.md` and `docs/v2.0/implementation/`. This document starts its own task numbering at the phase matching Spec M7, since there is no Phase 0 through Phase 6 work to redo: the foundation Phase 0 through Phase 4 established is unchanged, and Phase 7 below builds directly on the Phase 6 GUI as shipped.

**Existing code is the 2.0.0 release**, not a rewrite target. Every task below is new work; where a task reuses existing code unmodified (`Vault::entries()`, `Vault::extract`), its description says so and no separate line item exists for the reuse itself — Spec §5.1 already states plainly that FR-28 through FR-30 add no core surface.

---

## Conventions

**Task identifiers** are `P<phase>.<n>`, sequential within this document, continuing the numbering convention (not the numbers themselves) of the previous plan. They are not foundation identifiers — `HC`/`FR`/`A`/`C`/`S` belong to the suite and are only ever cited.

**Definition of done** for every task, without exception:
1. The behavior the cited requirement describes is observable.
2. Tests exist at the level the Spec's testing strategy (§9) prescribes for that kind of work.
3. `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `cargo test`, `cargo deny check`, `cargo audit` all pass locally (Spec §8.1). There is no CI; these gates run before every commit.

**Per-phase to-do lists and test cases** live in `docs/implementation/`, one pair per phase, each citing the requirement it verifies. They are written as each phase is reached, against this plan's task list for that phase — the same convention the previous plan used, in a fresh top-level folder since `docs/v2.0/implementation/` is that plan's own closed record.

---

## Phase 7 — Capability Surface (Spec M7, part 1)

*Proves the new capabilities exist and hold their own guarantees — no ciphertext read for a refused preview, no byte written to the vault directory for any of it — before anything is built on top of them in a webview, where the same proof is far more expensive to obtain.*

**Entry:** Phase 6 exit met (2.0.0 shipped). Foundation suite at Requirements v1.1, Design Guideline v1.3, Technical Specification v1.1.

| Task | Status | Work | Cites |
|---|---|---|---|
| P7.1 | Not yet built | `EntryInfo` (Tauri, `commands.rs`) gains `source_mtime`, serialized alongside the fields it already carries; no `veil-core` change, the field is already on `Entry` | FR-28, Spec §5.1, §5.3 |
| P7.2 | Not yet built | Extension derivation per FR-29's rule (substring of `name` after its last `.`, a leading dot not counting), implemented once in the CLI (Rust) and once in the GUI frontend (TypeScript); both checked against one shared fixture list covering `archive.tar.gz`, `.gitignore`, `README`, and the ordinary case | FR-29, Spec §5.1, §9 |
| P7.3 | Not yet built | `veil detail <vault> <file>` CLI subcommand: name, folder, size, `Modified` (source mtime), `Added`; table and JSON output; `NotFound` (13) when nothing matches the folder-and-name identity | FR-28, Spec §5.2 |
| P7.4 | Not yet built | `--group` on `list` becomes an optional-value flag: omitted stays flat (unchanged), bare stays folder-grouped, table output byte-identical (unchanged), `--group=extension` groups per FR-29. JSON is the one exception: it ignored `--group` entirely before this task, so a bare `--group --format json` now groups too, the same shape `--group=extension` uses | FR-29, Spec §5.2 |
| P7.5 | Not yet built | `preview_entry(id)` Tauri command: checks recorded `size` against C-5 before reading anything; for an entry within the cap and on the supported extension list, calls `Vault::extract` with a `Cursor<Vec<u8>>` sink and returns the typed `PreviewPayload` (`Image { mime, base64 }` / `Text { content }`) Spec §5.3 defines; an unsupported extension or invalid-UTF-8 text is refused before decryption | FR-30, Spec §5.3, C-5 |
| P7.6 | Not yet built | Integration test: a directory snapshot of the vault, taken before and after a `preview_entry` call, is byte-for-byte identical — the direct check for "memory only, never a temporary file" | FR-30, HC-2, Spec §9 |
| P7.7 | Not yet built | Test: an entry recorded larger than C-5 is refused, paired with a corrupted stand-in entry at that size, asserting the corruption is never reported — proof the refusal happens before any ciphertext is read | FR-30, C-5, Spec §9 |
| P7.8 | Not yet built | `assert_cmd` coverage for `detail` and `list --group`, including a bare `--group` invocation asserting identical output to before this phase | Spec §9, A-4 |

**Exit:**
- `veil detail` and `veil list --group[=extension]` are covered by CLI tests, table and JSON output both, and a bare `--group`'s table output regresses nothing (A-4, backward compatibility per Spec §5.2); its JSON output gains grouping it never had, which is the one documented exception to that guarantee.
- `preview_entry` refuses an oversized or unsupported entry without reading its stored ciphertext, and leaves the vault directory unchanged whether it succeeds or fails.
- The two independent extension-derivation implementations agree on every case in the shared fixture list.

---

## Phase 8 — Browsing Screen (Spec M7, part 2)

*Proves the product's browsing screen carries the new capabilities the way Design Guideline v1.3 specifies them — the context menu, the second grouping dimension, sort, multi-select, and preview — and ships as 2.1.0.*

**Entry:** Phase 7 exit met.

| Task | Status | Work | Cites |
|---|---|---|---|
| P8.1 | Not yet built | Grouping control becomes a three-way choice — none / folder / extension — replacing the boolean `group-toggle` checkbox; groups collapse and expand, per group, for the session; a collapsed header still shows its row count | FR-8, FR-29, Design §1.2, §3.2 |
| P8.2 | Not yet built | Column-header click-to-sort for all four columns (name, folder, size, added), ascending on first click, descending on the second, an arrow marking the active column and direction; sorts within each group when grouped | Design §3.2 |
| P8.3 | Not yet built | Multi-select — click, shift-click range, Cmd-click toggle — replacing the single `selectedId` state; Delete and the context menu act on the full selection | Design §3.2, §3.5 |
| P8.4 | Not yet built | Right-click context menu: Save as…, Show details, Preview (present only for a single, supported, in-cap selection), Replace… (single selection only), Delete | Design §3.5, FR-17, FR-22, FR-28, FR-30 |
| P8.5 | Not yet built | Details panel: FR-28's fields labeled as the list columns are, plus `Modified` distinct from `Added`; exact byte size; no content hash shown | FR-28, Design §8.9 |
| P8.6 | Not yet built | Preview overlay: header names the file; text content (including `.md`) shown unrendered in the body font; image content shown from the base64 payload; an over-cap or unsupported entry offers Save as… where Preview would be | FR-30, Design §8.10 |
| P8.7 | Not yet built | Preview and details state cleared — object URLs revoked, held strings released — on preview close, on lock (extends FR-3), and on application exit | FR-3, FR-30, Design §8.10, Spec §5.3 |
| P8.8 | Not yet built | A failed integrity check on opening a preview is worded identically to a failed extraction (Design §6) | FR-18, FR-30, Design §6, §8.10 |
| P8.9 | Not yet built | Vocabulary audit (Design §7) re-run across every new string this phase introduces — menu items, detail labels, preview chrome — in both the GUI and wherever the CLI's own wording overlaps | Design §7 |
| P8.10 | Not yet built | Version and packaging bump to 2.1.0 | Requirements §8, Spec §8 |

**Exit:** every item in the context menu works from a right-click with no other path required to reach it; grouping, sort, and multi-select behave exactly as Design §3.2 specifies; preview opens only for a supported, in-cap, single selection and leaves no content behind after closing, locking, or quitting; the vocabulary audit is clean; 2.1.0 is tagged.

---

## Cross-Cutting Obligations

These apply to every task in both phases and are part of the definition of done, not a final sweep:

- **No plaintext, key material, or password** reaches an error message, a `Debug` output, or a log line (HC-1, HC-2, Spec §6) — including preview content, which is new plaintext surface this round introduces.
- **Every new long-running operation** gets progress reporting and cooperative cancellation when it is written, not afterwards (A-3). Preview is not expected to need this at C-5's cap, but the check is not waived by that expectation.
- **Every new error variant or refusal** carries the state fact its message needs (Design §4.2, Spec §6) — an over-cap preview names the cap, an unsupported extension says so plainly.
- **Anything learned that changes HOW** goes into the Technical Specification as a version bump. This document records sequencing, never design.

---

## Sequencing Notes

- **Phase 7 before Phase 8**, for the same reason the previous plan put the CLI before durability work: a Tauri command's guarantees — no disk touch, no ciphertext read on refusal — are cheap to prove with an integration test and expensive to prove by driving a webview.
- **No format, crypto, or Phase 0–4 work reopens here.** Spec §10's M7 note states this outright; this plan's scope is the browsing screen and its immediate command-layer support, nothing beneath it.
- **P8.7's exit condition is coverage, not a memory guarantee.** Spec §5.3's own honesty clause applies: JavaScript offers no way to force immediate reclamation, so "cleared" is verified as "every code path that ends a preview calls the clearing routine," not by inspecting freed memory.

---

## Open Questions

- **Whether a base64 `Image` payload stays comfortable at C-5's 50 MiB cap**, or `preview_entry` should move to a dedicated byte-stream response. Resolver: P7.5, measured once built, per Spec §11.
- **Whether extension derivation (P7.2) should move into `veil-core`** if a third consumer ever needs it. Not blocking this plan; resolver: revisit if a third frontend appears (Spec §11).
- **Preview's keyboard shortcut, and a collapsed group's exact visual treatment** are explicitly open in Design Guideline §9, resolved in a future Design Guideline version informed by use of what Phase 8 ships — neither blocks this plan's exit.
