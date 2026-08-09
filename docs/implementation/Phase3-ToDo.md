# Veil2 — Phase 3 To-Do: Command-Line Application

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-09
**Owner:** wattanit
**Foundation and plan versions this list is built against:**
- Requirements Document v1.0 — upstream
- Design Guideline v1.0 — upstream
- Technical Specification v1.0 — upstream
- Implementation Plan v1.0 — upstream; this list expands Plan tasks P3.1–P3.9

This document owns the step-level breakdown of Phase 3. It defers what to build to the Requirements, how it presents to the Design Guideline, how it is built to the Specification, and phase sequencing to the Implementation Plan. Enumerated checks live in [Phase3-TestCases.md](Phase3-TestCases.md).

**This list supersedes the previous Phase 3 documents entirely.** The `reclaim-space` command and the representability-check call sites are removed; nothing replaces either.

---

## Conventions

**Item identifiers** are `P<phase>.<task>.<letter>`.

**Status** follows the Plan's convention: **built, carries forward** / **built, needs rewrite** / **built, remove entirely** / **not yet built**.

**Done** for an item means the cited behaviour is observable from a shell, and the test cases listed against it pass.

---

## What Phase 3 is for

Phase 2 built an API with no caller. Phase 3 is the caller, and it comes before durability work deliberately: crash-injection through a command is cheap, through a UI it is not. Parity is the requirement (A-4) — a capability reachable only from the GUI is a defect here.

The user-facing words are fixed by Design §7 and are not negotiable per command.

---

## P3.1 — The command surface

*Plan P3.1 · Spec §5.2 · Design §3.4, §7 · A-4, FR-23*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P3.1.a | Done | One command per core capability, spelled in Design §7's vocabulary — `reclaim-space` removed, since delete already frees the file | A-4, Design §7 | T3.1, T3.31 |
| P3.1.b | Built, carries forward | Files addressed by their stored path, never by an internal identifier | FR-13, Design §7 | T3.1 |
| P3.1.c | Built, carries forward | A path matching nothing reported as naming nothing, distinct from damage and from a wrong password | FR-2, HC-3, Spec §6 | T3.5 |
| P3.1.d | Built, carries forward | `add` refusing a path the vault already holds, naming it and pointing at `replace` | FR-14, Spec §6 | T3.4 |
| P3.1.e | Built, carries forward | `add` accepting both a file and a folder, reporting every path the walk declined | FR-9, FR-10, FR-11 | T3.1, T3.6 |
| P3.1.f | Built, carries forward | `list` filtering by folder and by name substring | FR-6, FR-7 | T3.8 |
| P3.1.g | Built, carries forward | No flag anywhere that schedules, times, or conditions an operation on a threshold | FR-23, Spec §5.2 | T3.3 |
| P3.1.h | Built, carries forward | The FR-27 disclosures attached to the moments that produce them: unrecoverability at `create`, the retained original after `add`, the persistence-until-deleted-immediately point at `delete`, the unprotected copy at `save-copy` | FR-27, HC-7, Design §7 | T3.19, T3.28, T3.29 |

---

## P3.1a — Checking for damage

*Plan P3.2 · Spec §4.8, §5.2 · FR-26, S-3*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P3.1a.a | Built, carries forward | `check` reading and authenticating every file, reporting each failure by name | FR-26, S-3 | T3.7 |
| P3.1a.b | Built, carries forward | A non-zero exit when any file fails | Spec §5.2, FR-26 | T3.7 |
| P3.1a.c | Built, carries forward | Failure reported per file and the check continuing | S-3 | T3.7 |
| P3.1a.d | Built, carries forward | A plain statement that Veil2 cannot recover a damaged file | Design §4.2, S-3 | T3.7 |

---

## P3.2 — Human output

*Plan P3.3 · Design §3.4, §7*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P3.2.a | Built, carries forward | A table in the GUI's column order, one row per file | Design §3.4 | T3.8 |
| P3.2.b | Built, carries forward | Sizes human-readable, counts exact and never rounded | Design §7 | T3.8, T3.12 |
| P3.2.c | Built, carries forward | Stored names printed exactly as stored, whatever the script | — | T3.9 |
| P3.2.d | Built, carries forward | Column widths computed on character count; misalignment for double-width scripts stated rather than hidden | Design §7 | T3.9 |
| P3.2.e | Done | `info` printing entry count and total size — no reclaimable figure, since there is none | FR-7 | T3.12 |

---

## P3.3 — Machine output

*Plan P3.4 · Design §3.4*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P3.3.a | Built, carries forward | A `--format json` mode carrying the same facts as the table, with exact byte counts | Design §3.4 | T3.10 |
| P3.3.b | Built, carries forward | The human default never machine-shaped | Design §3.4 | T3.11 |
| P3.3.c | Built, carries forward | Machine output on standard output alone, valid from first byte to last | Design §3.4 | T3.11, T3.20 |
| P3.3.d | Built, carries forward | Failures in machine mode reported in that mode | Design §3.4, §4.2 | T3.10 |

---

## P3.4 — Password input

*Plan P3.5 · Spec §5.2 · Design §3.4 · HC-2*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P3.4.a | Built, carries forward | Password read from a file named by `--password-file`, or from an environment variable | Spec §5.2 | T3.13, T3.14 |
| P3.4.b | Built, carries forward | No option anywhere that takes a password as an argument value | HC-2, Spec §5.2 | T3.2, T3.16 |
| P3.4.c | Built, carries forward | An interactive prompt with the input not echoed, used only when a terminal is attached | HC-2, Design §3.4 | T3.13 |
| P3.4.d | Built, carries forward | A non-interactive invocation with no password supplied fails immediately, naming the missing input | Design §3.4, Spec §5.2 | T3.15 |
| P3.4.e | Built, carries forward | One trailing newline trimmed from a password file, and nothing else | Spec §5.2 | T3.18 |
| P3.4.f | Built, carries forward | The second password of `create` and `password` supplied the same way, confirmed by re-entry when prompted interactively | FR-1, FR-4, C-4 | T3.19 |
| P3.4.g | Built, carries forward | A wrong password reported with its own exit code, distinct from a damaged vault | FR-2 | T3.17 |

---

## P3.5 — Progress and cancellation

*Plan P3.6 · Design §3.4 · A-3, FR-15, FR-19*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P3.5.a | Built, carries forward | Progress written to standard error, results to standard output | Design §3.4 | T3.20 |
| P3.5.b | Built, carries forward | An updating single line when standard error is a terminal | Design §3.4 | T3.21 |
| P3.5.c | Built, carries forward | Periodic lines with no control characters off-terminal | Design §3.4 | T3.21 |
| P3.5.d | Built, carries forward | An interrupt signal driving the core's cancel rather than killing the process | FR-15, FR-19, HC-4 | T3.22 |
| P3.5.e | Built, carries forward | A cancelled operation exiting with the cancelled code and stating what it left behind | FR-15, FR-19, Spec §6 | T3.22 |

---

## P3.6 — Exit codes

*Plan P3.7 · Spec §5.2, §6 · FR-2*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P3.6.a | Built, carries forward | One exit code per Spec §6 error class | Spec §5.2, §6, FR-2 | T3.23 |
| P3.6.b | Done | The mapping exhaustive over the error enum — code 14 (`NotRepresentable`) retired, nothing takes its place | Spec §6 | T3.23 |
| P3.6.c | Built, carries forward | The codes documented in `--help` | Design §3.4 | T3.23 |

---

## P3.7 — Removal

*Plan P3.8 · superseded*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P3.7.a | Done | `run.rs`'s `reclaim_space()` handler and `cli.rs`'s `Command::ReclaimSpace` | superseded | — |
| P3.7.b | Done | `check_representable`/`unrepresentable()` call sites in `run.rs` | superseded | — |
| P3.7.c | Done | `failure.rs`'s `Failure::NotRepresentable` | superseded | — |
| P3.7.d | Done | `examples/reclaim_subject.rs` | superseded | — |

---

## P3.8 — The test suite

*Plan P3.9 · Spec §9 · A-4*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P3.8.a | Built, carries forward | `assert_cmd` driving the built binary as a subprocess, over the full command surface | Spec §9, A-4 | all |
| P3.8.b | Built, carries forward | Every case asserting the exit code, not merely failure | Spec §5.2 | T3.23 |
| P3.8.c | Built, carries forward | Non-interactive cases run with no terminal attached | Design §3.4 | T3.15, T3.21 |
| P3.8.d | Built, carries forward | An audit over every command's help text and every message for the Design §7 forbidden words | Design §7 | T3.31 |
| P3.8.e | Built, carries forward | An audit that no output contains a password, key material, or file content | HC-2, Spec §6 | T3.32 |
| P3.8.f | Done | `tests/reclaim.rs` and `tests/representability.rs` | superseded | — |

---

## Coverage note

**FR-24 (changed on disk) is not reachable from the CLI.** A command-line invocation opens, writes, and exits — the generation it read is never stale by the time it commits, and a second writer is refused by the advisory lock first. The check is real and Phase 2 covers it (T2.5, T2.6); no CLI test provokes it, and none is written to.

**Interactive prompting (P3.4.c) is checked by hand**, not by the automated suite: a test harness has no terminal, and driving the prompt through a pseudo-terminal would reintroduce exactly the kind of untestable logic A-1 exists to prevent.

---

## Exit

Every core capability is reachable from the CLI; a scripted invocation with no tty succeeds; exit codes let a script tell a wrong password from a damaged vault without parsing text.

**`cargo check --workspace` is now clean everywhere except Phase 4's file.** `cargo check -p veil-cli --tests --examples` passes outright except `tests/crashes.rs` (Phase 4, untouched by design). All 6 CLI test binaries (`audits`, `codes`, `output`, `passwords`, `streams`, `surface` — 31 cases total) pass under `cargo test --release`. `cargo clippy -D warnings` and `cargo fmt --check` are clean across the crate.

**One test needed a content fix, not just a rewrite.** `codes.rs`'s delete-message test asserted the CLI said deleted bytes *stay in the vault until you reclaim space* — true under the old architecture, false now that delete frees the file immediately. It now asserts the opposite: the message names no persistence claim at all. `streams.rs`'s cancelled-add test asserted zero bytes were left in any pack; rewritten to check only what the vault's own API exposes, since the cancelled write's entry file may exist on disk as harmless residue (Spec §4.5) — the same fix already applied in Phase 2.

**Every `T3.x` case across the CLI test files was renumbered to match the current `Phase3-TestCases.md`**, since the source carried a different, older numbering (e.g. `codes.rs` had T3.19–T3.28 for what are now T3.7 and T3.23–T3.30; `passwords.rs`'s T3.33 is now T3.19). The CLI's `packs()`/`ruin()` test harness helpers became `entry_files()`/`ruin()`, operating on `entries/` instead of a `packs/` directory.

---

## Open Questions

None outstanding.
