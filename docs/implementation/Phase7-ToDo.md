# Veil2 — Phase 7 To-Do: Capability Surface

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-12
**Owner:** wattanit
**Foundation and plan versions this list is built against:**
- Requirements Document v1.1 — upstream
- Design Guideline v1.3 — upstream
- Technical Specification v1.1 — upstream
- Implementation Plan v1.0 — upstream; this list expands Plan tasks P7.1–P7.8

This document owns the step-level breakdown of Phase 7. It defers what to build to the Requirements, how it presents to the Design Guideline, how it is built to the Specification, and phase sequencing to the Implementation Plan. Enumerated checks live in [Phase7-TestCases.md](Phase7-TestCases.md).

---

## Conventions

**Item identifiers** are `P7.<task>.<letter>`.

**Status** follows the Plan's convention: **built, carries forward** / **built, needs rewrite** / **built, remove entirely** / **not yet built**. Everything in this phase is **not yet built** — there is no existing code this phase modifies, only new surface added beside it.

**Done** for an item means the cited behaviour is observable — from the CLI, or from a Tauri command called directly the way `tauri::test::mock_builder` already exercises the rest of `commands.rs` (Phase 5's own precedent) — and the test cases listed against it pass.

---

## What Phase 7 is for

Phase 6 shipped a GUI and CLI with nothing left to add to the browsing screen without a place to add it from. Phase 7 is that place: the Tauri commands, CLI subcommand, and flag change Phase 8 builds its interface against — proved on their own terms first, the same reason Phase 3 came before Phase 5/6: a refusal that never reads ciphertext, or a command that never touches the vault directory, is cheap to prove with an integration test and expensive to prove by driving a webview.

Nothing here touches the format, the crypto construction, or anything Phase 0 through 4 established. Spec §5.1 states plainly that FR-28 through FR-30 add no core surface; this phase is where that claim is exercised.

---

## P7.1 — Per-entry detail

*Plan P7.1, P7.3 · Spec §5.1–§5.3 · FR-28*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P7.1.a | Done | `EntryInfo` (`veil-gui/src/commands.rs`) gains `source_mtime`, serialised alongside the fields already there; no `veil-core` change | FR-28, Spec §5.1 | T7.1 |
| P7.1.b | Done | `veil detail <vault> <file>` table output: name, folder, exact byte size, `Modified` (source mtime), `Added` — identity argument matches `save-copy`/`replace`/`delete` | FR-28, Spec §5.2, Design §8.9 | T7.1 |
| P7.1.c | Done | `--format json` output for `detail`, same fields, exact byte integer | FR-28, Design §3.4 | T7.2 |
| P7.1.d | Done | No content hash in either output — the same decision Design §8.9 makes for the GUI panel, held here too so the two peers agree | FR-28, Design §8.9 | T7.1, T7.2 |
| P7.1.e | Done | `NotFound` (exit 13) when the folder-and-name identity matches nothing, reusing `Vault::find` (Spec §5.1) rather than a new lookup path | FR-28, FR-2, HC-3 | T7.3 |

---

## P7.2 — Extension derivation

*Plan P7.2 · Spec §5.1, §9 · FR-29*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P7.2.a | Not yet built | Rust implementation of FR-29's rule: substring of `name` after its last `.`, none if that `.` is the first character or nothing follows it (a trailing dot), lowercased for comparison | FR-29 | T7.4 |
| P7.2.b | Not yet built | TypeScript implementation of the same rule, as a small pure module (`crates/veil-gui/ui/src/extension.ts`) exporting one function — no other frontend change in this phase, since the grouping control it feeds is Phase 8's work | FR-29 | T7.4 |
| P7.2.c | Not yet built | One shared fixture list of name → expected-extension pairs, read by both implementations' tests rather than duplicated by hand into two literals | FR-29, Spec §9 | T7.4 |
| P7.2.d | Not yet built | A Rust test (`crates/veil-gui/tests/extension_parity.rs`) that runs the TypeScript function under `node` against the fixture list and diffs its output against the Rust implementation's — mechanical parity with no new frontend test-framework dependency, the same proportionality call Phase 5 made declining a WebDriver suite | FR-29, Spec §7, §9 | T7.4 |

---

## P7.3 — The `--group` flag

*Plan P7.4 · Spec §5.2 · FR-29*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P7.3.a | Not yet built | `--group` becomes `clap`'s optional-value form: omitted stays flat, bare defaults to `folder`, `--group=extension` is new | FR-29, Spec §5.2 | T7.5, T7.6, T7.7 |
| P7.3.b | Not yet built | Table output grouped by extension, group label lowercased, one reserved group for entries with none | FR-29, Design §3.2 | T7.5 |
| P7.3.c | Not yet built | JSON output carries the same grouping | FR-29, Design §3.4 | T7.5 |

---

## P7.4 — The preview command

*Plan P7.5 · Spec §5.3 · FR-30, C-5*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P7.4.a | Not yet built | `preview_entry(id)` checks the entry's recorded `size` against C-5 before any read | FR-30, C-5 | T7.10 |
| P7.4.b | Not yet built | Extension checked against the supported list before decryption is attempted | FR-30 | T7.9 |
| P7.4.c | Not yet built | `Vault::extract` called with `dst` a `Cursor<Vec<u8>>` — no new core method, per Spec §5.1 | FR-30, FR-17, FR-18 | T7.8, T7.12 |
| P7.4.d | Not yet built | `PreviewPayload` (`Image { mime, base64 }` / `Text { content }`) per Spec §5.3's definition | FR-30 | T7.8 |
| P7.4.e | Not yet built | A text-listed extension whose decrypted bytes are not valid UTF-8 is refused, not returned lossily | FR-30 | T7.11 |

---

## P7.5 — Preview's guarantees, proved directly

*Plan P7.6, P7.7 · Spec §9 · FR-30, HC-2, C-5*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P7.5.a | Not yet built | Directory-snapshot test: the vault directory's contents, taken before and after a `preview_entry` call, are byte-for-byte identical | FR-30, HC-2 | T7.13, T7.14 |
| P7.5.b | Not yet built | Refusal paired with a corrupted stand-in entry — over C-5, and on an unsupported extension — asserting the corruption is never surfaced, proving the read never happened | FR-30, C-5, HC-3 | T7.9, T7.10 |

---

## P7.6 — CLI test coverage and vocabulary

*Plan P7.8 · Spec §9 · A-4, Design §7*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P7.6.a | Not yet built | `assert_cmd` coverage for `detail`, found and not-found, table and JSON | Spec §9, A-4 | T7.1, T7.2, T7.3 |
| P7.6.b | Not yet built | `assert_cmd` coverage for `list --group`, omitted / bare / `=extension`, including a regression case asserting bare `--group` is byte-identical to its pre-Phase-7 output | Spec §9, A-4 | T7.5, T7.6, T7.7 |
| P7.6.c | Not yet built | Every string this phase adds — `detail`'s and `--group`'s help text, every refusal message — searched against Design §7's forbidden words and fixed vocabulary | Design §7 | T7.16 |

---

## P7.7 — Disclosure audit

*Cross-Cutting Obligations · HC-1, HC-2, Spec §6*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P7.7.a | Not yet built | No error, `Debug` output, or log line from any `preview_entry` path (success, refusal, or failure) contains previewed file content | HC-2, Spec §6 | T7.15 |
| P7.7.b | Not yet built | `tracing` output for `preview_entry` names the operation only — no entry name, folder, or content, the same discipline every other command already holds to | HC-1, Spec §6 | T7.15 |

---

## Coverage note

**The TypeScript half of extension derivation (P7.2.b) is exercised only by P7.2.d's parity test in this phase.** Its use inside the browsing screen's grouping control does not exist yet — that is Phase 8's own work and Phase 8's own coverage.

**Preview's frontend-side memory clearing is out of scope here.** There is no frontend consumer of `preview_entry` in this phase, so there is nothing yet that could hold a stale reference. Design §8.10's clearing obligation is proved where it is built, in Phase 8 (P8.7).

**FR-24 remains unreachable from the CLI**, for the same reason Phase 3's ToDo already recorded: a command-line invocation opens, writes, and exits before its own generation could go stale.

---

## Exit

`veil detail` and `veil list --group[=extension]` are covered by CLI tests, table and JSON output both, and a bare `--group` regresses nothing. `preview_entry` refuses an oversized or unsupported entry without reading its stored ciphertext, and leaves the vault directory unchanged whether it succeeds or fails. The two independent extension-derivation implementations agree on every case in the shared fixture list. No new dependency was added on either side, per Spec §7.

---

## Open Questions

- **Whether a base64 `Image` payload stays comfortable at C-5's 50 MiB cap.** Not blocking; observed directly when T7.8 runs against an image near the cap. Resolver: the Plan's own open item, revisited if the measurement is uncomfortable.
