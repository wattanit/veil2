# Veil2 — Phase 2 To-Do: Vault Operations

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-09
**Owner:** wattanit
**Foundation and plan versions this list is built against:**
- Requirements Document v1.0 — upstream
- Design Guideline v1.0 — upstream
- Technical Specification v1.0 — upstream
- Implementation Plan v1.0 — upstream; this list expands Plan tasks P2.1–P2.17

This document owns the step-level breakdown of Phase 2. It defers what to build to the Requirements, how it presents to the Design Guideline, how it is built to the Specification, and phase sequencing to the Implementation Plan. Enumerated checks live in [Phase2-TestCases.md](Phase2-TestCases.md).

**This list supersedes the previous Phase 2 documents entirely.** Statistics, delete, and damage attribution are rewritten around one-file-per-entry storage; compaction and representability have no place here at all.

---

## Conventions

**Item identifiers** are `P<phase>.<task>.<letter>`.

**Status** follows the Plan's convention: **built, carries forward** / **built, needs rewrite** / **built, remove entirely** / **not yet built**.

**Done** for an item means the cited behaviour is observable, the test cases listed against it pass, and the Plan's definition of done holds.

---

## What Phase 2 is for

Phase 1 proved the format. Phase 2 proves the API — sufficient for both frontends, before either exists. Most of it is already built; what changed with the storage architecture is delete (immediate file removal, no reclaim accounting), statistics (derived by summing on call, not maintained), and damage attribution (a direct per-entry file check, no pack indirection). Two modules are removed outright: compaction (`vault/reclaim.rs`) and the extraction representability check (`vault/representable.rs`) — neither has a requirement behind it anymore.

---

## P2.1 — Create, open, lock, and the advisory lock

*Plan P2.1 · Spec §2, §5.1 · FR-1, FR-2, FR-3, FR-24, A-7*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P2.1.a | Built, carries forward | An advisory lock taken on a lock file at open and held for the lifetime of the open vault | FR-23, Spec §2 | T2.1 |
| P2.1.b | Built, carries forward | A second opener told the vault is in use, by name | FR-23, Spec §6 | T2.1 |
| P2.1.c | Built, carries forward | The lock released when the vault is closed, including on an error path and on unwind | FR-23, HC-4 | T2.2 |
| P2.1.d | Built, carries forward | `lock` consuming the vault and zeroising every key it holds | FR-3, HC-2, Spec §5.1 | T2.3 |
| P2.1.e | Built, carries forward | A write refused when the index generation on disk is ahead of the one held in memory | FR-24, Spec §4.3, §4.4 | T2.5 |
| P2.1.f | Built, carries forward | Nothing in the open path stored in a process-global, so two vaults may be open in one process | A-7, Spec §2 | T2.4 |
| P2.1.g | Built, carries forward | A reload that adopts an external change using the keys already held | FR-24 | T2.6 |

---

## P2.2 — The index at open

*Plan P2.2 · Spec §4.3, §5.1 · FR-7, S-2*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P2.2.a | Built, carries forward | The whole index decrypted at open and held in memory; browsing thereafter reads no file | FR-7, S-2 | T2.4 |
| P2.2.b | Done | Statistics computed by summing the resident entry list, never scanned from disk | FR-7 | T2.27 |
| P2.2.c | Built, carries forward | Open touching no entry file, so open cost tracks entry count and not vault size | S-2 | T2.4 |

---

## P2.3 — Progress and cancellation

*Plan P2.3 · Spec §2 · A-3, FR-15, FR-20*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P2.3.a | Built, carries forward | A progress sink passed as a parameter, never global, with a no-op implementation the CLI can pass | A-3, Spec §2 | T2.7 |
| P2.3.b | Built, carries forward | A cancellation token passed as a parameter, shareable across threads | A-3, Spec §2 | T2.7 |
| P2.3.c | Built, carries forward | Cancellation checked at chunk boundaries | Spec §2, FR-15 | T2.9 |
| P2.3.d | Built, carries forward | Progress reported for every long operation: ingest, extraction, folder ingest, verification | FR-15, FR-20, Spec §4.8 | T2.7 |
| P2.3.e | Built, carries forward | Cancellation reported as its own outcome carrying whether the operation rolled back | FR-15, Spec §6 | T2.8 |

---

## P2.4 — Ingest

*Plan P2.4 · Spec §4.7 · FR-9, FR-12*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P2.4.a | Built, carries forward | The source opened read-only, never modified, moved, or unlinked | FR-9, Spec §4.7 | T2.11 |
| P2.4.b | Done | Content streamed into `entries/{id}.entry`, fsynced with its containing directory, before the index generation that references it advances | FR-12, Spec §4.5, §4.7 | T2.12 |
| P2.4.c | Built, carries forward | Success reported only after the index write returns | FR-12 | T2.12 |
| P2.4.d | Done | A cancelled or failed ingest advances no generation; the partial entry file is left as unreferenced residue, per Spec §4.5 | FR-15, Spec §4.7 | T2.8 |
| P2.4.e | Built, carries forward | Entry identifiers never reused, including after delete, after emptying the vault, and across a reopen — the counter stored rather than derived | Spec §3.2, §4.3 | T2.25 |
| P2.4.f | Built, carries forward | The full path — folder and name together — is the entry's identity, compared exactly | FR-13, Spec §4.6 | T2.20 |

---

## P2.5 — Folder ingest

*Plan P2.5 · Spec §4.7 · FR-10, FR-11*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P2.5.a | Built, carries forward | A walk over regular files only, storing each with its path relative to the added root as folder metadata | FR-10, FR-8 | T2.13 |
| P2.5.b | Built, carries forward | Symbolic links not followed, at every level | FR-11 | T2.14 |
| P2.5.c | Built, carries forward | Each skipped link recorded and returned to the caller | FR-11 | T2.14 |
| P2.5.d | Built, carries forward | A walk that terminates on a tree containing a link cycle | FR-11 | T2.15 |
| P2.5.e | Built, carries forward | Path separators normalised to `/` in the stored folder field regardless of host | Spec §4.6 | T2.13 |

---

## P2.6 — Extraction

*Plan P2.6 · Spec §4.7 · FR-16, FR-17, FR-19, FR-20*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P2.6.a | Built, carries forward | Output written to a caller-supplied `Write`, so no path is chosen inside the core | FR-16, S-1, HC-2 | T2.19 |
| P2.6.b | Done | Content read directly from `entries/{id}.entry`; a missing file is reported as damage to that entry, with no pack-existence indirection | FR-17, S-3 | T2.17 |
| P2.6.c | Built, carries forward | The content hash compared after the final chunk, and failure named with the entry | FR-17, S-3 | T2.17 |
| P2.6.d | Built, carries forward | Partial output removed on any failure | FR-17, HC-3 | T2.17 |
| P2.6.e | Built, carries forward | Partial output removed on cancellation as well as on failure | FR-19, FR-17 | T2.10 |
| P2.6.f | Built, carries forward | Peak memory independent of entry size in both directions | FR-20, S-1 | T2.18 |

---

## P2.7 — Replace

*Plan P2.7 · Spec §4.6, §4.7 · FR-13, HC-4*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P2.7.a | Built, carries forward | The target matched on folder and name together | FR-13, Spec §4.6 | T2.20 |
| P2.7.b | Built, carries forward | New content written under a new id and durable before any index generation advances | FR-13, HC-4, Spec §4.7 | T2.21 |
| P2.7.c | Built, carries forward | One generation step that simultaneously repoints the path and drops the old id; the old entry file removed afterward | FR-13 | T2.22 |
| P2.7.d | Built, carries forward | A replace whose ingest fails leaves the previous entry intact and reachable | HC-4, FR-13 | T2.21 |

---

## P2.8 — Delete

*Plan P2.8 · Spec §4.5 · FR-22*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P2.8.a | Built, carries forward | The entry removed from the index and immediately unreachable through the API | FR-22 | T2.23 |
| P2.8.b | Done | The index write fsynced, then the entry's file removed — never the reverse | FR-22, Spec §4.5 | T2.24 |
| P2.8.c | Done | Reclaimable-bytes accounting on delete — no requirement supports it; a deleted entry's space is already freed | superseded | T2.24 |

---

## P2.9 — Statistics

*Plan P2.9 · Spec §4.3, §5.1*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P2.9.a | Done | Entry count and total size computed by `Statistics::from_entries(&self.document.entries)` on every call; `IndexDocument.statistics` is no longer a persisted field at all, and `Vault::recount_statistics()` is removed | Spec §4.3, §5.1 | T2.26 |
| P2.9.b | Done | The incremental updates to those removed fields in ingest, replace, and delete | superseded | T2.26 |

---

## P2.10 — Limits

*Plan P2.10 · FR-16, C-1, C-2*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P2.10.a | Built, carries forward | An addition beyond the entry limit refused, naming the limit and the current value | FR-16, C-1 | T2.28 |
| P2.10.b | Built, carries forward | An entry beyond the file-size limit refused, naming the limit and the actual size | FR-16, C-2 | T2.29 |
| P2.10.c | Built, carries forward | The size limit enforced during the stream, not only from a stated length | FR-16, C-2 | T2.29 |
| P2.10.d | Built, carries forward | A refused addition advances no generation | FR-16, HC-4 | T2.28, T2.29 |

---

## P2.11 — Password change

*Plan P2.11 · Spec §3.1 · FR-4*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P2.11.a | Built, carries forward | A new KEK derived from the new password with a fresh salt, and the master key rewrapped under it | FR-4, Spec §3.1 | T2.30 |
| P2.11.b | Built, carries forward | No content, index, or entry key touched — the wrapped master key and header fields are the whole change | FR-4, A-6 | T2.30, T2.31 |
| P2.11.c | Built, carries forward | The old password verified before anything is written | FR-4, FR-2 | T2.33 |
| P2.11.d | Built, carries forward | A failed or interrupted change leaves the vault openable with the old password | HC-4, FR-4 | T2.33 |

---

## P2.12 — Damage attribution

*Plan P2.12 · Spec §5.1 · S-3*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P2.12.a | Done | `vault/damage.rs`'s `missing_packs()`/`referenced_packs()` pack-walk | superseded | — |
| P2.12.b | Done | `unreadable_entries()` — one file-existence check per entry, no content read | Spec §5.1, S-3 | T2.41 |

---

## P2.13 — Remove compaction

*Plan P2.13 · superseded*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P2.13.a | Done | `vault/reclaim.rs` in full — `compact()`, `Candidate`/`Reclaimed`, garbage-ratio pack selection, copy-forward. Nothing replaces it: delete already frees the file | superseded | — |

---

## P2.14 — Remove the representability check

*Plan P2.14 · superseded*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P2.14.a | Done | `vault/representable.rs` in full — `check_representable()`, `RESERVED_NAMES`, reserved-character and case-collision checks | superseded | — |

---

## P2.15 — Integration tests

*Plan P2.15 · Spec §9 · A-1*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P2.15.a | Built, carries forward | The full lifecycle driven through the public API with no process, no terminal, no prompt | A-1, Spec §9 | T2.34 |
| P2.15.b | Done | Shared test harness — `create()` takes no cap parameter; `flip_byte_in_pack()` replaced by `flip_byte_in_entry_file()`; `assert_statistics_match_recount()` replaced by `assert_statistics_correct()` | Spec §9 | all |
| P2.15.c | Done | `tests/reclaim.rs`, `tests/representability.rs`, `tests/portability_fixture.rs`, and their example generators | superseded | — |

---

## P2.16 — Property tests

*Plan P2.16 · Spec §9*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P2.16.a | Built, carries forward | Arbitrary byte sequences at arbitrary lengths, including zero, surviving ingest and extraction byte-identically | Spec §9, FR-16 | T2.35 |
| P2.16.b | Done | Arbitrary sequences of add, replace, and delete leaving statistics equal to a direct sum, not a recount | FR-22, Spec §9 | T2.36 |

---

## P2.17 — Whole-vault verification

*Plan P2.17 · Spec §4.8 · FR-26, S-3*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P2.17.a | Built, carries forward | Verification reusing the extraction path with output discarded | FR-26, Spec §4.8 | T2.37 |
| P2.17.b | Built, carries forward | Nothing written during verification, no exclusive lock required | Spec §4.8 | T2.40 |
| P2.17.c | Done | A failing entry recorded and verification continuing, returning every failure by name — a missing entry file is reported the same way as any other damage | FR-26, S-3 | T2.38 |
| P2.17.d | Built, carries forward | Progress reported per entry rather than per byte | Spec §4.8, Design §8.6 | T2.7 |
| P2.17.e | Built, carries forward | Cancellation returning the entries verified so far and their results | Spec §4.8, FR-15 | T2.39 |
| P2.17.f | Built, carries forward | Never scheduled, never automatic, never triggered at open | FR-26 | T2.4 |

---

## Exit

- The full lifecycle runs with no terminal present (T2.34).
- Statistics computed after an arbitrary sequence of add, replace, and delete match a direct sum over `entries()` (T2.26, T2.36).
- A cancelled ingest leaves a vault indistinguishable from one where it never began, at the level the API exposes (T2.8).
- Password change completes in time independent of vault size (T2.30, T2.31).

**`cargo check --workspace` is clean everywhere this phase owns.** `cargo check -p veil-core --lib --examples` passes outright. All 21 non-Phase-4 `veil-core` test binaries pass (`cargo test -p veil-core --test <name>`, run individually since `tests/durability.rs` still fails to build and blocks a combined `cargo test`). `crates/veil-core/tests/durability.rs` remains broken — Phase 4's file, untouched by design. `crates/veil-cli` remains broken — Phase 3's crate, untouched by design (11 errors, all from the now-removed `reclaim`/`representable` API).

**Two Phase 1-era test assertions turned out to be too strict for the new residue model, and were corrected here rather than left failing:** `limits_password.rs`'s T2.29 and `progress_cancel.rs`'s T2.7 both asserted the vault directory was *byte-identical* after a refused or cancelled write. Under one-file-per-entry storage this is no longer true by design — `stage()` has no rollback, so a refused or cancelled write leaves its entry file behind as harmless, unreferenced residue (Spec §4.5). Both tests now exclude `.entry` files from the directory comparison and check the index-visible state (entries/statistics/generation) instead, which is what the requirement actually guarantees.

---

## Open Questions

- **Carried from Phase 1:** the Argon2id measurement against C-3 on the weakest supported hardware.
