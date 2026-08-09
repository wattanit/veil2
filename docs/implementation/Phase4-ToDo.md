# Veil2 — Phase 4 To-Do: Durability

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-09
**Owner:** wattanit
**Foundation and plan versions this list is built against:**
- Requirements Document v1.0 — upstream
- Design Guideline v1.0 — upstream
- Technical Specification v1.0 — upstream
- Implementation Plan v1.0 — upstream; this list expands Plan tasks P4.1–P4.5

This document owns the step-level breakdown of Phase 4. It defers what to build to the Requirements, how it presents to the Design Guideline, how it is built to the Specification, and phase sequencing to the Implementation Plan. Enumerated checks live in [Phase4-TestCases.md](Phase4-TestCases.md).

**This list supersedes the previous Phase 4 documents entirely, and the phase is smaller than it was.** Compaction is gone — there is no pack to garbage-collect, no working-space bound to prove, no interrupted-compaction recovery to design. What remains is HC-4 over `add`, `replace`, and `delete`, which one-file-per-entry storage makes considerably simpler to reason about than the pack-based version this phase used to prove.

---

## Conventions

**Item identifiers** are `P<phase>.<task>.<letter>`.

**Status** follows the Plan's convention: **built, carries forward** / **built, needs rewrite** / **built, needs review** / **not yet built**.

**Done** for an item means the cited behaviour is observable, the test cases listed against it pass, and the Plan's definition of done holds.

---

## What Phase 4 is for

Phases 1 through 3 built a vault that works when nothing goes wrong. Phase 4 proves HC-4: no single interruption leaves a vault that cannot be opened, and none destroys the only copy of data the vault held before the operation began.

The CLI comes first for this phase's benefit: crash-injection through a command is cheap, through a UI it is not.

---

## P4.1 — Write ordering

*Plan P4.1 · Spec §4.7 · HC-4, FR-12*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P4.1.a | Built, needs review | Every write path in `veil-core` re-enumerated against the entry-file layout — one file, one containing directory, one index generation, no rollover | Spec §4.7, FR-12 | T4.1 |
| P4.1.b | Built, carries forward | A file's containing directory made durable after the file is created, renamed over, or removed | HC-4, FR-12 | T4.1, T4.2 |
| P4.1.c | Built, needs review | Ingest, replace, and delete each written in the order Spec §4.5 fixes: content durable, then the index generation that names it, then (for delete) the file removed | Spec §4.5, FR-12 | T4.2–T4.4 |
| P4.1.d | Built, carries forward | No indirection layer, no injectable filesystem, no test hook — the ordering is checked by killing a process, or it is not checked | Spec §9 | T4.2–T4.4 |

---

## P4.2 — Crash tests

*Plan P4.2 · Spec §9 · HC-4*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P4.2.a | Built, carries forward | A real process killed with an uncatchable signal, part-way through an operation genuinely in flight | Spec §9, HC-4 | T4.2–T4.4 |
| P4.2.b | Built, carries forward | The kill triggered by watching the vault's own bytes appear on disk, not by anything the process was built to tell a test | Spec §9, §11 | T4.2 |
| P4.2.c | Built, carries forward | Four invariants asserted after every kill: the vault opens; every file that existed beforehand is still listed; each extracts byte-identically; and the statistics match a direct sum | HC-4, FR-7 | T4.5 |
| P4.2.d | Built, needs rewrite | Add, replace, and delete killed through the shipped binary. There is nothing left to kill mid-compaction | Spec §9 | T4.2–T4.4 |
| P4.2.e | Built, carries forward | A deterministic set that runs with the suite, and a repeated randomised sweep marked `#[ignore]`, run on request | Spec §9, §8.1 | T4.6 |
| P4.2.f | Built, carries forward | The signal is a kill, not an interrupt — an interrupt is cancellation and a different guarantee, already covered by T3.22 | FR-15, HC-4 | T4.2 |

---

## P4.3 — A missing entry file

*Plan P4.3 · Spec §4.5 · S-3*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P4.3.a | Built, needs review | A missing entry file does not prevent the vault from opening — verified under crash conditions, not only by construction | Spec §4.5, S-3 | T4.9 |
| P4.3.b | Built, carries forward | The affected entry is named without reading content | S-3, S-2 | T4.9 |
| P4.3.c | Built, carries forward | Every entry outside it still listed, still extractable, still verified | S-3 | T4.10 |
| P4.3.d | Built, carries forward | Reported as damage in the words Design §7 fixes, pointing at `check` for the full list | Design §4.2, §7, FR-26 | T4.9 |

---

## P4.4 — Nothing at open

*Plan P4.4 · Spec §4.5 · HC-4, FR-24*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P4.4.a | Built, carries forward | **Nothing at open.** No write — not the index, not an entry file — and no walk of `entries/` | HC-4, S-2, FR-24 | T4.11 |
| P4.4.b | Built, carries forward | A file the index does not reference — residue of a replace or delete interrupted between its two steps — is left alone: reported nowhere, swept by nothing | HC-4, Spec §4.5 | T4.8 |
| P4.4.c | Built, carries forward | This costs at most one entry's worth of space and is visible to anyone who looks at the directory directly | Spec §4.5 | T4.8 |

There is no reclaim mechanism to hand this residue to, and none is built. This is a deliberate product decision: an index that is momentarily behind its own directory is indistinguishable from this case by construction, and removing a file on that guess risks the loss HC-4 forbids.

---

## P4.5 — Read-only vaults

*Plan P4.5 · Spec §4.5, §4.8 · FR-23*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P4.5.a | Built, carries forward | Read-only vaults open read-only and say so at open, rather than at the first failed write | Spec §4.5, §4.8, FR-23 | T4.12 |

---

## Exit

- No interruption at any fsync boundary yields an unopenable vault or loses an entry that existed beforehand — the kill is a process kill, not a power cut (T4.2–T4.7).
- Opening a vault writes nothing and measures nothing, with the generation unchanged (T4.11).
- A vault on read-only media opens, and says so at open (T4.12).

---

## Open Questions

- **Whether the platform's `fsync` reaches the platter.** The write ordering is proved by crash-injection; whether the underlying platform honours `fsync` all the way to the medium needs whole-machine power loss, and there is no rig for it. Acknowledged gap, not closed.
