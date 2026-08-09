# Veil2 — Implementation Plan

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-09
**Owner:** wattanit
**Foundation versions this plan is built against:**
- Requirements Document v1.0 — upstream
- Design Guideline v1.0 — upstream
- Technical Specification v1.0 — upstream

This document owns the **sequencing** of the work: ordered phases expanding the Technical Specification's milestones (Spec §10), each with entry and exit conditions, and each task citing the foundation item that put it there. It defers what to build to the Requirements, how it presents to the Design Guideline, and how it is built to the Specification. No task below restates a format, an algorithm, or a layout — each cites the Spec section that defines it. If implementation finds the Spec wrong or underspecified, that is a Specification version bump, not a correction recorded here.

**This plan supersedes the previous Implementation Plan entirely.** The foundation suite it was built against is gone: Requirements, Design Guideline, and Technical Specification were each rewritten around a narrower motivation, and the storage architecture changed from pack files with compaction to one file per entry. No task or numbering from the previous plan carries forward.

**Existing code predates this plan.** A large part of `veil-core` and `veil-cli` was built against the old architecture before the rewrite. Rather than treat that code as unknown, every task below states its build status against what actually exists today:

- **Built, carries forward** — already implemented, matches this plan without change. The phase's own tests and exit condition are the review; no separate task is listed for these beyond what is already in the phase.
- **Built, needs rewrite** — implemented, but against packs, compaction, or the withdrawn portability apparatus. The task names what changes.
- **Built, remove entirely** — implemented to serve a requirement that no longer exists. The task names what gets deleted, with nothing replacing it.
- **Not yet built** — new work.

---

## Conventions

**Task identifiers** are `P<phase>.<n>`, sequential within this document. They are not foundation identifiers — `HC`/`FR`/`A`/`C`/`S` belong to the suite and are only ever cited.

**Definition of done** for every task, without exception:
1. The behavior the cited requirement describes is observable.
2. Tests exist at the level the Spec's testing strategy (§9) prescribes for that kind of work.
3. `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `cargo test`, `cargo deny check`, `cargo audit` all pass locally (Spec §8.1). There is no CI; these gates run before every commit.

**Per-phase to-do lists and test cases** live in `docs/implementation/`, one pair per phase, each citing the requirement it verifies. They are written as each phase is reached, against this plan's task list for that phase.

---

## Phase 0 — Workspace and Gate Foundation

*Proves nothing about the product; makes every later proof possible.*

**Entry:** foundation suite approved at v1.0.

| Task | Status | Work | Cites |
|---|---|---|---|
| P0.1 | Built, carries forward | Cargo workspace with `veil-core`, `veil-cli`, `veil-gui`; module skeleton | Spec §1, A-1, A-4 |
| P0.2 | Built, needs rewrite | Error taxonomy — `Damaged::Pack` replaced by an entry-file-scoped damage variant (§4.5 makes attribution trivial: a missing file names its own entry, no pack indirection needed); `Unrepresentable` and `Error::NameNotRepresentable` removed, along with the CLI's `Failure::NotRepresentable` and exit code 14 | Spec §6, FR-2, FR-5, FR-6 |
| P0.3 | Built, carries forward | Key-material newtypes with `ZeroizeOnDrop` and hand-written `Debug` that print a placeholder | Spec §3.1, §6, HC-2 |
| P0.4 | Built, carries forward | `cargo deny` and `cargo audit` gating the build; dependency versions pinned | Spec §7, HC-6 |
| P0.5 | Built, carries forward | Logging guard: a test asserting that entry names, folder metadata, and content never reach `tracing` output | Spec §6, HC-1 |

**Exit:** every gate of the definition of done passes locally, and each is confirmed to reject a deliberate violation of itself.

---

## Phase 1 — Format and Crypto Core (Spec M1)

*Proves the format and the cryptographic construction, and that tampering and truncation fail loudly.*

**Entry:** Phase 0 exit met.

| Task | Status | Work | Cites |
|---|---|---|---|
| P1.1 | Built, carries forward | Argon2id KEK derivation reading parameters from the header, never from constants | Spec §3.1, §4.2, HC-5, HC-6, C-3 |
| P1.2 | Built, carries forward | Master-key generation and AEAD wrapping with the whole header as associated data | Spec §3.1, HC-5, HC-7, A-6 |
| P1.3 | Built, carries forward | HKDF-SHA256 subkey derivation with versioned `info` strings | Spec §3.1, HC-6 |
| P1.4 | Built, carries forward | Header serialisation, magic, and read-time dispatch on `format_version` | Spec §4.2, HC-5, FR-5, FR-6 |
| P1.5 | Built, carries forward | STREAM content encryption and decryption, `Read` → `Write`, entry id bound as associated data | Spec §3.3, HC-3, A-2, S-1 |
| P1.6 | Built, carries forward | BLAKE3 content hashing computed in the same pass as encryption | Spec §3.3, §4.7, FR-18 |
| P1.7 | Built, needs rewrite | Entry model and CBOR index serialisation — remove the `Extent` type and `IndexDocument.next_pack_id`; `next_entry_id` and `generation` stay | Spec §4.3, FR-6 |
| P1.8 | Built, carries forward | Double-buffered index persistence with generation counter; write to the older slot, fsync, highest authenticating generation wins | Spec §4.4, HC-4, FR-24 |
| P1.9 | Built, remove entirely | `store/pack.rs`'s `PackSink`/`PackSource`, pack rollover, extent bookkeeping, `next_pack_id` allocation, and `store/mod.rs`'s pack-facing exports | Spec §4.1, §4.5 (superseded) |
| P1.10 | Not yet built | Entry-file write and read: one file per entry under `entries/`, named by its id, holding exactly that entry's STREAM-encrypted chunks | Spec §4.1, §4.5, A-5 |
| P1.11 | Built, carries forward | NFC normalisation on ingest, exact case-sensitive comparison thereafter | Spec §4.6 |
| P1.12 | Built, needs rewrite | End-to-end vertical slice: create a vault, store one file, read it back byte-identically — exercised over the entry-file path (P1.10) rather than packs | Spec §4.1–§4.5 |
| P1.13 | Built, needs rewrite | **Adversarial corruption suite** — every row of the Spec §9 table, with "corrupt one entry's file, fail only that entry" (S-3) replacing the old per-pack case | Spec §9, HC-3, S-3 |
| P1.14 | Partially done | Argon2id cost measured at 0.27s on development hardware (Spec §11); measure against C-3's one-second target on the **weakest** supported hardware and record the result | C-3, Spec §11 |

**Exit — hard gate:**
- Round-trip is byte-identical for empty, single-chunk, and multi-chunk files.
- **Every mutation in the Spec §9 corruption table fails as required**, including the truncated-final-chunk case.
- A corrupted entry file fails **only** that entry, and names it (S-3).
- Argon2id parameters are measured and recorded on the weakest available hardware.

**No work from Phase 2 begins until P1.13 is green.**

---

## Phase 2 — Vault Operations (Spec M2)

*Proves the core API is sufficient for both frontends, before either exists.*

**Entry:** Phase 1 exit met, corruption suite green.

| Task | Status | Work | Cites |
|---|---|---|---|
| P2.1 | Built, carries forward | `create` / `open` / `lock`, advisory lock held for the vault's lifetime, and the write-time generation check that makes FR-24's counter a detector rather than a number | Spec §2, §5.1, FR-1, FR-2, FR-3, FR-24, A-7 |
| P2.2 | Built, carries forward | Index loaded and decrypted at open, presenting every entry with its metadata without touching stored content; browsing thereafter serves from memory | Spec §4.3, §5.1, FR-7, S-2 |
| P2.3 | Built, carries forward | Progress sink and cancellation token as parameters, checked at chunk boundaries | Spec §2, A-3, FR-15, FR-20 |
| P2.4 | Built, needs rewrite | Ingest pipeline — internals stream into `entries/{id}.entry` (P1.10) instead of a pack; the fsync ordering (file, then containing directory, then index) is unchanged | Spec §4.7, FR-9, FR-12 |
| P2.5 | Built, carries forward | Folder walk over regular files only; symlinks not followed, recorded as skipped | Spec §4.7, FR-10, FR-11 |
| P2.6 | Built, needs rewrite | Extraction — internals open `entries/{id}.entry` directly instead of seeking through pack extents; a missing file is reported as damage to that entry directly, no pack-existence check | Spec §4.7, FR-17, FR-18, FR-21 |
| P2.7 | Built, carries forward at the API level | Replace matched on full path — folder and name together — writes new content under a new id, fsyncs it, then one index generation step repoints the path and drops the old id | Spec §4.6, §4.7, FR-13 |
| P2.8 | Built, needs rewrite | Delete — index removal, fsync, **then remove the entry's file immediately**; no reclaimable-bytes accounting | Spec §4.5, FR-22 |
| P2.9 | Built, needs rewrite | Statistics — entry count and total size computed by summing the resident `entries()` list on call; remove `Statistics.physical_bytes`/`reclaimable_bytes`, the incremental updates to them in ingest/replace/delete, and `Vault::recount_statistics()` | Spec §4.3, §5.1 |
| P2.10 | Built, carries forward | Limit enforcement naming both the limit and the actual value | FR-16, C-1, C-2 |
| P2.11 | Built, carries forward | Password change rewrapping the master key only | Spec §3.1, FR-4 |
| P2.12 | Built, needs rewrite | `unreadable_entries()` — replace `vault/damage.rs`'s pack-walk (`missing_packs`, `referenced_packs`) with one file-existence check per entry, no content read | Spec §5.1, S-3 |
| P2.13 | Built, remove entirely | `vault/reclaim.rs` in full — `compact()`, `Candidate`/`Reclaimed`, garbage-ratio pack selection, copy-forward into a fresh pack. Nothing replaces it: deleting an entry already frees its file (P2.8) | superseded — no requirement supports it |
| P2.14 | Built, remove entirely | `vault/representable.rs` in full — `check_representable()`, `RESERVED_NAMES`, reserved-character and case-collision checks. HC-8 is withdrawn; nothing checks representability on another platform | superseded — no requirement supports it |
| P2.15 | Built, needs rewrite | Integration tests against `veil-core` directly — the shared harness (`tests/harness/mod.rs`) drops its pack-cap parameter from `create()` and replaces `flip_byte_in_pack()` with a direct entry-file corruption helper; `tests/reclaim.rs`, `tests/representability.rs`, `tests/portability_fixture.rs`, and their example generators are deleted outright | Spec §9, A-1 |
| P2.16 | Built, carries forward | Property tests: any byte sequence at any length survives round-trip | Spec §9 |
| P2.17 | Built, needs rewrite | Whole-vault verification over the extraction path with output discarded; internals inherit P2.6's rewrite | Spec §4.8, FR-26, S-3 |

**Exit:**
- The full lifecycle runs with no terminal present, which is A-1 made observable.
- Statistics computed after an arbitrary sequence of add, replace, and delete match a direct sum over `entries()` — there is no separate figure to recount against.
- A cancelled ingest leaves a vault indistinguishable from one where it never began (FR-15).
- Password change completes in time independent of vault size (FR-4).

---

## Phase 3 — Command-Line Application (Spec M3)

*Proves the core is usable with no UI, and establishes the integration surface everything later depends on.*

**Entry:** Phase 2 exit met.

| Task | Status | Work | Cites |
|---|---|---|---|
| P3.1 | Built, needs rewrite | `clap` surface covering every core capability — remove `Command::ReclaimSpace`; there is nothing left to reclaim once delete is immediate (P2.8) | A-4, Spec §5.2 |
| P3.2 | Built, carries forward | Verification command exiting non-zero when any entry fails | Spec §4.8, §5.2, FR-26 |
| P3.3 | Built, carries forward | Human-readable table output in the GUI's column order | Design §3.4 |
| P3.4 | Built, carries forward | Machine-readable output mode for scripting | Design §3.4 |
| P3.5 | Built, carries forward | Password input from environment variable or file; never from a command-line argument; non-interactive invocation detected and failed with the missing input named | Spec §5.2, HC-2 |
| P3.6 | Built, carries forward | Progress to stderr, results to stdout, degrading to periodic lines off-terminal | Design §3.4 |
| P3.7 | Built, needs rewrite | Exit codes per Spec §5.2's table — retire code 14 (`NotRepresentable`); the remaining table is otherwise unchanged | Spec §5.2, §6, FR-2 |
| P3.8 | Built, remove entirely | `run.rs`'s `reclaim_space()` handler, its `check_representable`/`unrepresentable()` call sites, `report.rs`'s pack-walk-based `info` formatting (superseded by P2.9), and `examples/reclaim_subject.rs` | superseded — no requirement supports it |
| P3.9 | Built, needs rewrite | `assert_cmd` suite over the full command surface — drop `tests/reclaim.rs` and `tests/representability.rs` outright; update the rest to the new harness | Spec §9, A-4 |

**Exit:** every core capability is reachable from the CLI; a scripted invocation with no tty succeeds; exit codes let a script tell a wrong password from a damaged vault without parsing text (FR-2).

---

## Phase 4 — Durability (Spec M4)

*Proves HC-4 — that no single interruption leaves a vault unopenable or loses data that existed beforehand.*

**Entry:** Phase 3 exit met. The CLI comes first deliberately: crash-injection is far cheaper to drive through a command than through a UI.

| Task | Status | Work | Cites |
|---|---|---|---|
| P4.1 | Built, needs review | Audit every write path against the fsync ordering the Spec prescribes — re-checked against the entry-file path (P1.10, P2.4), which is simpler than the pack path it replaces: one file, one directory, one index generation, no rollover | Spec §4.7, HC-4, FR-12 |
| P4.2 | Built, needs rewrite | Crash tests that kill a real process mid-operation, for `add`, `replace`, and `delete`. The existing suite (`tests/crashes.rs`, `tests/durability.rs`) also kills mid-compaction; that scenario is deleted, since compaction no longer exists | Spec §9, HC-4 |
| P4.3 | Built, needs review | A missing entry file is damage to exactly that entry, and to no other — verified under crash conditions rather than only by construction (P2.12) | Spec §4.5, S-3 |
| P4.4 | Built, carries forward | **Nothing at open**: no write, no walk of `entries/`. A file the index no longer references, left by a replace or delete interrupted between its two steps, is left alone — reported nowhere, swept by nothing | Spec §4.5, FR-24, HC-4 |
| P4.5 | Built, carries forward | Read-only vaults open read-only and say so at open | Spec §4.5, §4.8, FR-23 |

**Exit:**
- No interruption at any fsync boundary yields an unopenable vault or loses an entry that existed beforehand — the kill is a process kill, not a power cut (Spec §9).
- Opening a vault writes nothing and measures nothing, with the generation unchanged (FR-24, S-2).
- A vault on read-only media opens, and says so at open rather than at the first failed write (FR-23).

---

## Phase 5 — GUI Foundation (Spec M5)

*Proves that the interface renders the user's own filenames correctly — the one thing it exists to do — before any feature is built on it.*

**Entry:** Phase 4 exit met.

| Task | Status | Work | Cites |
|---|---|---|---|
| P5.1 | Not yet built | Tauri v2 shell over `veil-core`; operations on a worker thread, progress marshalled to the UI thread | Spec §5.3, A-3, A-4 |
| P5.2 | Not yet built | Ephemeral webview storage configured (ordinary configuration; see the note below) | Spec §5.3, HC-1 |
| P5.3 | Not yet built | CSP restricted to the bundled origin; no `localStorage`, `sessionStorage`, or IndexedDB; devtools compiled out of release | Spec §5.3, HC-1 |
| P5.4 | Not yet built | Virtualised entry list at the density and typography the design fixes, including tabular numerals | Design §2.3, §3.2 |
| P5.5 | Not yet built | Complex-script rendering verified in both themes — the evidence that decided the toolkit | Design §2.2 |
| P5.6 | Not yet built | Whole-window drop target naming the count before release; native file dialogs | Design §3.3, FR-9, FR-17 |

**Note on webview persistence:** Spec §5.3 treats this as ordinary configuration, not a gated feature — the worst a lapse leaks is filenames, not content. P5.2 covers it once, with no dedicated test suite and no per-platform release gate.

**Exit:** Thai, Arabic, Han, and emoji filenames render correctly in light and dark. Dropping 34 files shows "34" before release.

---

## Phase 6 — GUI v1 (Spec M6)

*Proves the product.*

**Entry:** Phase 5 exit met.

| Task | Status | Work | Cites |
|---|---|---|---|
| P6.1 | Not yet built | Unlock screen — four elements only, a visibly alive working state during derivation, wrong password and damaged vault as distinct outcomes | Design §5, FR-2, C-3 |
| P6.2 | Not yet built | Superseded and too-new format messages | Design §5, FR-5, FR-6 |
| P6.3 | Not yet built | Vault creation: password subject to C-4, the unrecoverability block, explicit acknowledgement rather than a pre-ticked box | Design §8.2, HC-7, C-4, FR-1, FR-27 |
| P6.4 | Not yet built | Identity bar with lock state legible at a glance, and the statistics line | Design §3.2, FR-7 |
| P6.5 | Not yet built | Search and the folder-grouping view toggle — a view control, not a tree | Design §3.2, FR-8 |
| P6.6 | Not yet built | Add flow with progress and cancel, and the retained-originals clause on completion | Design §8.3, FR-9, FR-15, FR-27 |
| P6.7 | Not yet built | Extract flow: destination always chosen, overwrite confirmed by name, the unprotected-copy line every time | Design §6, FR-17, FR-19, FR-27 |
| P6.8 | Not yet built | Delete with the persistence clause, and the count named in the confirmation | Design §8.4, FR-22, FR-27 |
| P6.9 | Not yet built | Lock action and a locked screen distinct from a greyed-out list | Design §8.5, FR-3, HC-1 |
| P6.10 | Not yet built | Three-part error presentation — what happened, what state things are in, what you can do | Design §4.2 |
| P6.11 | Not yet built | Constrained conditions: vault in use, read-only, changed on disk, storage gone, destination full, limits exceeded, damaged entries marked per-entry | Design §4.3, FR-16, FR-23, FR-24, FR-25, S-3 |
| P6.12 | Not yet built | Damage check: time estimate before starting, per-entry progress, cancellation returning partial results, a result that names failing files and states plainly that Veil cannot recover them | Design §8.6, FR-26, S-3 |
| P6.13 | Not yet built | Vocabulary audit against the Design §7 table across GUI and CLI alike | Design §7 |
| P6.14 | Not yet built | Packaging for macOS: bundle UTI, signing, notarisation | Spec §8.2 |
| P6.15 | Not yet built | The release states the platform it was run on | Requirements §2.1, §8 |

**Exit:** every functional requirement is reachable from the GUI; the vocabulary audit is clean in both applications; the macOS package installs, opens a vault, and is the 2.0.0 release.

---

## Cross-Cutting Obligations

These apply to every task in every phase and are part of the definition of done, not a final sweep:

- **No plaintext, key material, or password** reaches an error message, a `Debug` output, or a log line (HC-1, HC-2, Spec §6).
- **Every new long-running operation** gets progress reporting and cooperative cancellation when it is written, not afterwards (A-3).
- **Every new error variant** carries the state fact the three-part message needs (Design §4.2, Spec §6).
- **Anything learned that changes HOW** goes into the Technical Specification as a version bump. This document records sequencing, never design.

---

## Sequencing Notes

- **P1.13 gates everything.** The corruption suite is not a Phase 1 deliverable to be finished later; it is the condition for starting Phase 2.
- **CLI before durability work.** Crash-injection through a command is cheap; through a GUI it is not.
- **Argon2id cost measured on the weakest target** (P1.14), not the development machine. A vault that cannot be opened on a modest laptop is a worse failure than a slow derivation on a fast one.
- **No portability phase, no second-platform phase.** Requirements §2.1 scopes this release to macOS only, and §2.2 defers Windows and Linux without a scheduled path. There is nothing in this plan for either.

---

## Open Questions

- **Exact Argon2id cost parameters on the weakest supported hardware.** Resolver: P1.14, when that hardware is available.
- **Maximum length of the path metadata recorded under FR-10.** Resolver: Phase 1, before the header/index fields it would constrain are finalised.
- **Whether Phase 6 ships as one release or the GUI lands incrementally behind a pre-release tag.** Affects nothing technical; affects when the 2.0.0 tag is cut. Resolver: owner, at Phase 5 exit.
