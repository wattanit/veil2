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
| P7.2.a | Done | Rust implementation of FR-29's rule (`crates/veil-cli/src/extension.rs`): substring of `name` after its last `.`, none if that `.` is the first character or nothing follows it (a trailing dot), lowercased for comparison | FR-29 | T7.4 |
| P7.2.b | Done | TypeScript implementation of the same rule, as a small pure module (`crates/veil-gui/ui/src/extension.ts`) exporting one function — no other frontend change in this phase, since the grouping control it feeds is Phase 8's work | FR-29 | T7.4 |
| P7.2.c | Done | One shared fixture list of name → expected-extension pairs, living once in `tests/extension_parity.rs`, checked against both implementations there rather than copied into a second literal | FR-29, Spec §9 | T7.4 |
| P7.2.d | Done | A Rust test, relocated to `crates/veil-cli/tests/extension_parity.rs` (not `veil-gui` as first planned — the Rust half of the rule lives in `veil-cli`, since that is its actual consumer via `--group`, so the parity check belongs beside it) — a minimal `src/lib.rs` was added to `veil-cli` so the integration test could call `extension_of` directly rather than through a subprocess. It bundles `extension.ts` with the frontend's own `esbuild` (no new dependency) and runs the result under `node` | FR-29, Spec §7, §9 | T7.4 |

---

## P7.3 — The `--group` flag

*Plan P7.4 · Spec §5.2 · FR-29*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P7.3.a | Done | `--group` becomes `clap`'s optional-value form (`num_args = 0..=1`, `require_equals = true`, `default_missing_value = "folder"`): omitted stays flat, bare defaults to `folder`, `--group=extension` is new. Table output for a bare `--group` is unchanged byte-for-byte | FR-29, Spec §5.2 | T7.5, T7.6, T7.7 |
| P7.3.b | Done | Table output grouped by extension, group label lowercased, one reserved group (`(no extension)`) for entries with none | FR-29, Design §3.2 | T7.5 |
| P7.3.c | Done | JSON output carries the same grouping (`{"groups": [{"group": ..., "files": [...]}]}`, `group: null` for the no-extension bucket) — **new for a bare `--group` too**: JSON ignored `--group` entirely before this task and always printed a flat listing, undocumented and untested, so this closes a gap rather than preserving one. Built from the same `output::group_key` the table renders by, so the two output modes cannot disagree about what a group is | FR-29, Design §3.4 | T7.5 |

---

## P7.4 — The preview command

*Plan P7.5 · Spec §5.3 · FR-30, C-5*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P7.4.a | Done | `preview_entry(id)` (`crates/veil-gui/src/preview.rs`) checks the entry's recorded `size` against C-5 (`MAX_PREVIEW_BYTES`) before any read | FR-30, C-5 | T7.10 |
| P7.4.b | Done | Extension checked against the supported list (`classify`, a third small implementation of FR-29's rule, for the reason recorded in the module doc — a closed eleven-entry lookup, not general grouping) before decryption is attempted | FR-30 | T7.9 |
| P7.4.c | Done | `Vault::extract` called with `dst` a `Cursor<Vec<u8>>` — no new core method, per Spec §5.1 | FR-30, FR-17, FR-18 | T7.8, T7.12 |
| P7.4.d | Done | `PreviewPayload` (`Image { mime, base64 }` / `Text { content }`) per Spec §5.3's definition. Base64 is hand-written (RFC 4648, checked against its own test vectors) rather than a new dependency | FR-30 | T7.8 |
| P7.4.e | Done | A text-listed extension whose decrypted bytes are not valid UTF-8 is refused (`PreviewNotText`), not returned lossily | FR-30 | T7.11 |

**Also done here, ahead of P7.7:** `tests/vocabulary.rs`'s audit (T6.31) never scanned `veil-gui/src` itself — only `ui/src` and `veil-cli/src` — even though `ErrorInfo.message` text reaches the frontend unescaped in more than one fallback path. `preview.rs` is the first place in this crate's own Rust source to author user-facing prose directly rather than relay `veil_core::Error`'s `Display` text, so the gap became load-bearing; extended the audit to cover `veil-gui/src`, which caught one pre-existing violation in `commands.rs` (`replace_entry`'s internal-bug message said "entry", now "file") — fixed rather than left for later.

---

## P7.5 — Preview's guarantees, proved directly

*Plan P7.6, P7.7 · Spec §9 · FR-30, HC-2, C-5*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P7.5.a | Done | Directory-snapshot test (`snapshot_all`, a recursive path→bytes map — stronger than comparing names or sizes): the vault directory's contents, taken before and after a `preview_entry` call, are byte-for-byte identical, for a successful call (T7.13) and for each refusal/failure case (T7.14) | FR-30, HC-2 | T7.13, T7.14 |
| P7.5.b | Done | Already satisfied by P7.4's own T7.9/T7.10: each pairs its refusal with a corrupted stand-in entry, so a wrong refusal order would surface as `Corrupt` instead of the expected kind | FR-30, C-5, HC-3 | T7.9, T7.10 |

---

## P7.6 — CLI test coverage and vocabulary

*Plan P7.8 · Spec §9 · A-4, Design §7*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P7.6.a | Done | `assert_cmd` coverage for `detail`, found and not-found, table and JSON | Spec §9, A-4 | T7.1, T7.2, T7.3 |
| P7.6.b | Done | `assert_cmd` coverage for `list --group`, omitted / bare / `=extension`, including a case confirming bare `--group`'s table output is byte-identical to its pre-Phase-7 form and a case confirming its JSON output is not (P7.3.c) | Spec §9, A-4 | T7.5, T7.6, T7.7 |
| P7.6.c | Done | **No separate mechanism written.** `tests/audits.rs`'s existing T3.31/T3.32 already scan every command's `--help` text and a run over the whole surface including failure paths (Design §7); `detail`, `detail`'s not-found case, and `list --group=extension` were added to what they scan, in both audits, rather than building a second scanner for the same thing. One real finding from running it: the `GroupBy` enum's own doc comments — clap's source for its `--help` "Possible values" text — said "each entry's...", which T3.31 correctly failed on; fixed to "each file's..." | Design §7 | T3.31, T3.32 |

---

## P7.7 — Disclosure audit

*Cross-Cutting Obligations · HC-1, HC-2, Spec §6*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P7.7.a | Done | No error or `Debug` output from any `preview_entry` refusal or failure path (unsupported, over-cap, damaged) contains the previewed file's content or name — checked with both markers embedded, the file's stored bytes ruined first so a wrong result would prove the read happened anyway. Success is out of scope for this item: its payload legitimately *is* the content (T7.8 already checks it's exactly the original bytes) | HC-2, Spec §6 | T7.15 |
| P7.7.b | Done | **Read literally, this crate has no "discipline" to hold to — it calls no `tracing` macro anywhere and does not depend on the crate.** Proved by construction rather than by capturing output: a source-and-manifest scan (the same class of check `tests/structure.rs`'s T5.6 already makes for persistent storage APIs) asserting no `tracing` reference exists in `veil-gui/Cargo.toml` or `veil-gui/src`. If a future phase instruments this crate, that scan starts failing and this item's proof has to be redone as a real capture, the way `veil-core`'s own logging guard works | HC-1, Spec §6 | T7.15 |

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

- **Whether a base64 `Image` payload stays comfortable at C-5's 50 MiB cap — genuinely still open.** T7.8 exercises the `Image` path with a 200-byte fixture, for correctness; T7.10's near-cap fixture is a `.txt` file refused before any read, so it never produces a payload at all. No test in this phase has actually built and passed a base64 payload anywhere near 50 MiB. Not blocking Phase 7's exit, since C-5's cap is a Requirements value independent of how comfortable it turns out to be — but a real measurement (a large fixture image, once one exists) belongs to Phase 8, when a frontend is actually receiving these payloads over IPC. Resolver: the Plan's own open item.

## Phase 7 exit

**Met.** P7.1 through P7.7 are done; every gate (`cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `cargo test` across all three crates, `cargo deny check`, `cargo audit`) passes, and no new dependency was added on either side. Two corrections surfaced along the way rather than being caught in review afterward: a live vocabulary-audit failure in the `GroupBy` enum's own `--help` text (P7.3), and a pre-existing gap in `tests/vocabulary.rs` that never scanned `veil-gui/src` at all, which caught one unrelated leftover violation in `commands.rs` once closed (P7.4). Phase 8 — the browsing screen itself — can now build against a capability surface that has already been proved to hold its own guarantees, per the Plan's own reason for sequencing it this way.
