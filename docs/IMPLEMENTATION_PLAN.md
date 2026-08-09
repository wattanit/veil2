# Veil2 — Implementation Plan

**Version:** 2.0
**Status:** approved
**Date:** 2026-08-09
**Owner:** wattanit
**Foundation versions this plan is built against (G-14):**
- Requirements Document **v2.0** — upstream
- Design Guideline **v2.0** — upstream
- Technical Specification **v2.0** — upstream

**Downstream documents:**
- [Phase 0 To-Do](implementation/Phase0-ToDo.md) v1.0 · [Phase 0 Test Cases](implementation/Phase0-TestCases.md) v1.0
- [Phase 1 To-Do](implementation/Phase1-ToDo.md) v1.0 · [Phase 1 Test Cases](implementation/Phase1-TestCases.md) v1.0
- [Phase 2 To-Do](implementation/Phase2-ToDo.md) v1.0 · [Phase 2 Test Cases](implementation/Phase2-TestCases.md) v1.0
- [Phase 3 To-Do](implementation/Phase3-ToDo.md) v1.0 · [Phase 3 Test Cases](implementation/Phase3-TestCases.md) v1.0
- [Phase 4 To-Do](implementation/Phase4-ToDo.md) v2.0 · [Phase 4 Test Cases](implementation/Phase4-TestCases.md) v2.0 — approved; its ten upstream notes resolved into Requirements v2.0 and Specification v2.0, one of them by withdrawing the requirement rather than absorbing the note

*Changes since v1.6 (**major — the suite it is pinned to reversed two decisions**):* re-pinned to Requirements v2.0, Design Guideline v2.0 and Specification v2.0, which resolve all ten of Phase 4's notes. Two things move in this document as a result:

**Phase 5 stops depending on hardware that does not exist.** Requirements §2.1 now ships macOS at 2.0 with Windows and Linux following as their own releases, so the tasks one machine can finish stay in Phase 5 and the two that need a second machine become **Phase 8**. This is the change that unblocks the plan: Phase 5's exit condition was previously a portability run across three platforms, which nobody could perform, and a phase that cannot be exited is a phase the work eventually walks around.

**Phase 4's exit is met outright rather than in part, and the condition itself is gone.** It required an interrupted compaction to be cleaned up at next open. FR-32 is withdrawn: nothing happens at open, the space is found by the next reclaim or the next request for the figures, and the condition below says that instead.

> **This document's earlier versions are wrong, including v2.0 as first published earlier the same day**, which re-pinned to a suite that had reversed FR-32 rather than withdrawn it. Phases 0 to 3's to-do lists and test cases remain pinned to the versions they were built against — that is the as-built record and is deliberate — but every FR-32 reference in them describes a requirement that no longer exists.

Also: P2.1 gains the FR-27 citation that Phase 2's first upstream note asked for and that v1.4 recorded as absorbed without making; P5.7 is added for the `next_pack_id` field Specification §4.3 now defines, and was built immediately rather than deferred, because an unreleased format is the only time that field is free.

*Changes since v1.4 (minor — additive, no decision reversed):* the Phase 3 to-do list and test cases are written and pinned above; re-pinned to the v1.2 Requirements, v1.2 Design Guideline and v1.3 Specification, which absorbed what those two documents raised.

*Changes since v1.3 (minor):* re-pinned to Specification v1.2. P0.4 is withdrawn and the definition of done no longer claims CI — there is no pipeline and none is wanted. P5.3 and P5.5 become manual and on-request rather than scheduled jobs.

*Changes since v1.2 (minor — additive, no decision reversed):* the Phase 2 to-do list and test cases are written and pinned above.

*Changes since v1.1 (minor — additive, no decision reversed):* the open question on when per-phase to-do lists are written is resolved and moved to the ledger; the Phase 0 and Phase 1 documents it resolves are pinned above.

*Changes since v1.0:* re-pinned to the v1.1 suite; the three blocked tasks are unblocked; verification scheduled into Phases 2, 3 and 7.

This document owns the **sequencing** of the work: ordered phases expanding the Technical Specification's milestones (Spec §10), each with entry and exit conditions, and each task citing the foundation item that put it there. It defers what to build to the Requirements, how it presents to the Design Guideline, and how it is built to the Specification.

**It is not a shadow spec (G-11).** No task below restates a format, an algorithm, or a layout — each cites the Spec section that defines it. If implementation discovers that something in the Spec is wrong or underspecified, that flows back as a **Specification version bump** through the feedback protocol, never as a correction recorded here.

---

## Conventions

**Task identifiers** are `P<phase>.<n>` — section numbering for reference within this document. They are not foundation identifiers; the standard's `HC`/`FR`/`A`/`C`/`S` categories are reserved for the suite (G-19).

**Definition of done** for every task, without exception:
1. The behavior the cited requirement describes is observable.
2. Tests exist at the level the Spec's strategy (§9) prescribes for that kind of work.
3. `cargo clippy` clean, `cargo fmt` applied, `cargo deny` and `cargo audit` passing.
4. The local gates of Spec §8.1 pass: `fmt --check`, `clippy -D warnings`, `test`, `deny check`, `audit`.

This used to read "CI green on all three platforms". There is no CI and nothing runs on three platforms, so that clause was never met by any task and is removed. The gates run on macOS, which is what 2.0 ships (Requirements §2.1). Every task is still written for all three — Spec §8.1 forbids writing for one platform while accepting verification on one — and Phase 8 is where the other two are run.

**Enumerated test cases live in per-phase test-case documents** (G-10), not here. This plan names what a phase must prove; the test-case documents enumerate the individual checks, each citing the requirement it verifies.

**Per-phase to-do lists and test cases live in `docs/implementation/`**, written two phases ahead of the work rather than all at once — see the resolved entry in Open Questions. Phase 0 and Phase 1 exist; the rest are written as their phases approach.

---

## Phase 0 — Workspace and Gate Foundation

*Proves nothing about the product; makes every later proof possible.*

**Entry:** foundation suite approved at v1.1.

| Task | Work | Cites |
|---|---|---|
| P0.1 | Cargo workspace with `veil-core`, `veil-cli`, `veil-gui`; module skeleton | Spec §1, A-1, A-4 |
| P0.2 | Error taxonomy skeleton — the variants of Spec §6 defined, `anyhow` excluded from `veil-core` by lint | Spec §6, FR-2, FR-5, FR-30 |
| P0.3 | Key-material newtypes with `ZeroizeOnDrop` and hand-written `Debug` that print a placeholder | Spec §3.1, §6, HC-2 |
| ~~P0.4~~ | **Withdrawn** — a three-platform CI matrix. There is no CI pipeline and none is wanted (Spec §8.1); the same gates run locally. Number retained, not reused | Spec §8.1, HC-8 |
| P0.5 | `cargo deny` and `cargo audit` gating the build; dependency versions pinned | Spec §7, HC-6 |
| P0.6 | Logging guard: a test asserting that entry names, folder metadata, and content never reach `tracing` output | Spec §6, HC-1 |

**Exit:** every gate of the definition of done passes locally, and each is confirmed to reject a deliberate violation of itself. A deliberately-added test that logs an entry name **fails** — P0.6 is worthless unless it can be shown to fire.

---

## Phase 1 — Format and Crypto Core (Spec M1)

*Proves the format and the cryptographic construction, and that tampering and truncation fail loudly.*

**Entry:** Phase 0 exit met.

| Task | Work | Cites |
|---|---|---|
| P1.1 | Argon2id KEK derivation reading parameters from the header, never from constants | Spec §3.1, §4.2, HC-5, HC-6, C-3 |
| P1.2 | Master-key generation and AEAD wrapping with the whole header as associated data | Spec §3.1, HC-5, HC-7, A-6 |
| P1.3 | HKDF-SHA256 subkey derivation with versioned `info` strings | Spec §3.1, HC-6 |
| P1.4 | Header serialisation, magic, and read-time dispatch on `format_version` | Spec §4.2, HC-5, FR-5, FR-30 |
| P1.5 | STREAM content encryption and decryption, `Read` → `Write`, entry id bound as associated data | Spec §3.3, HC-3, A-2, S-1 |
| P1.6 | BLAKE3 content hashing computed in the same pass as encryption | Spec §3.3, §4.7, FR-17 |
| P1.7 | Entry model and CBOR index serialisation tolerant of unknown fields | Spec §4.3, FR-30 |
| P1.8 | Double-buffered index persistence with generation counter; write to the older slot, fsync, highest authenticating generation wins | Spec §4.4, HC-4, FR-27 |
| P1.9 | Pack file write and read with extents and the size cap | Spec §4.5, S-3, S-4, A-5, FR-25 |
| P1.10 | End-to-end vertical slice: create a vault, store one file, read it back byte-identically — the first point at which header, index, packs and content encryption are proven to compose | Spec §4.1–§4.5 |
| P1.11 | **Adversarial corruption suite** — every row of the Spec §9 table | Spec §9, HC-3, S-4 |
| P1.12 | Measure Argon2id cost against C-3's one-second target on the **weakest** supported hardware; record the chosen values | C-3, Spec §11.1 |

**Exit — and this is a hard gate:**
- Round-trip is byte-identical for empty, single-chunk, and multi-chunk files.
- **Every mutation in the Spec §9 corruption table fails as required**, including the truncated-final-chunk case that the original Veil accepted while reporting success.
- A corrupted pack fails **only** the entries with extents in it, and names them (S-4).
- Argon2id parameters are measured and recorded, resolving that Spec open item.

**No work from Phase 2 begins until P1.11 is green.** Building product on an unverified crypto core is exactly how the original shipped a silent data-loss bug, and every later phase inherits whatever is wrong here.

---

## Phase 2 — Vault Operations (Spec M2)

*Proves the core API is sufficient for both frontends, before either exists.*

**Entry:** Phase 1 exit met, corruption suite green.

| Task | Work | Cites |
|---|---|---|
| P2.1 | `create` / `open` / `lock`, advisory lock held for the vault's lifetime, and the write-time generation check that makes FR-27's counter a detector rather than a number. `Vault` is an instance value carrying no process-global state, so the single-vault limit stays a product decision rather than a structural one | Spec §2, §5.1, FR-1, FR-2, FR-3, FR-26, **FR-27**, A-7 |
| P2.1a | Index loaded and decrypted at open, presenting every entry with its metadata without touching stored content; browsing thereafter serves from memory | Spec §4.3, §5.1, FR-6, S-2 |
| P2.2 | Progress sink and cancellation token as parameters, checked at chunk boundaries | Spec §2, A-3, FR-14, FR-19 |
| P2.3 | Ingest pipeline with copy semantics and the fsync ordering that makes durability true | Spec §4.7, FR-9, FR-12 |
| P2.4 | Folder walk over regular files only; symlinks not followed, recorded as skipped | Spec §4.7, FR-10, FR-11 |
| P2.5 | Extraction to a caller `Write`, verified, partial output removed on failure | Spec §4.7, FR-16, FR-17, FR-20 |
| P2.6 | Replace matched on full path — folder and name together — with new content durable before the old becomes unreachable | Spec §4.6, §4.7, FR-13 |
| P2.7 | Delete as index removal plus reclaimable accounting | Spec §4.5, FR-21 |
| P2.8 | Statistics maintained incrementally, never scanned | Spec §4.3, FR-8, FR-22 |
| P2.9 | Limit enforcement naming both the limit and the actual value | FR-15, C-1, C-2 |
| P2.10 | Password change rewrapping the master key only | Spec §3.1, FR-4 |
| P2.11 | Integration tests driving `veil-core` directly — no process, no terminal | Spec §9, A-1 |
| P2.12 | Property tests: any byte sequence at any length survives round-trip | Spec §9 |
| P2.13 | Whole-vault verification over the extraction path with output discarded; continues past a failing entry and returns every failure | Spec §4.8, FR-33, S-4 |

**Exit:**
- The full lifecycle runs with no terminal present, which is A-1 made observable.
- Statistics match a full recount after an arbitrary sequence of add, replace, and delete.
- A cancelled ingest leaves a vault indistinguishable from one where it never began (FR-14).
- Password change completes in time independent of vault size (FR-4).

---

## Phase 3 — Command-Line Application (Spec M3)

*Proves the core is usable with no UI, and establishes the integration surface everything later depends on.*

**Entry:** Phase 2 exit met.

| Task | Work | Cites |
|---|---|---|
| P3.1 | `clap` surface covering every core capability — parity is the requirement, not a goal. Compaction carries no scheduling flag, timer, or threshold switch | A-4, FR-23, Spec §5.2 |
| P3.1a | Verification command exiting non-zero when any entry fails, so it is usable in a backup script without parsing output | Spec §4.8, §5.2, FR-33 |
| P3.2 | Human-readable table output in the GUI's column order | Design §3.4 |
| P3.3 | Machine-readable output mode for scripting | Design §3.4 |
| P3.4 | Password input from environment variable or file; **never** from a command-line argument; non-interactive invocation detected and failed with the missing input named | Spec §5.2, HC-2 |
| P3.5 | Progress to stderr, results to stdout, degrading to periodic lines off-terminal | Design §3.4 |
| P3.6 | Exit codes distinguishing the Spec §6 error classes | Spec §5.2, §6, FR-2 |
| P3.7 | `assert_cmd` suite over the full command surface | Spec §9, A-4 |

**Exit:** every core capability is reachable from the CLI; a scripted invocation with no tty succeeds; exit codes let a script tell a wrong password from a damaged vault without parsing text (FR-2).

---

## Phase 4 — Durability and Compaction (Spec M4)

*Proves HC-4 and FR-25 — the properties that make a vault trustworthy at hundreds of gigabytes.*

**Entry:** Phase 3 exit met. The CLI comes first deliberately: crash-injection is far cheaper to drive through a command than through a UI.

| Task | Work | Cites |
|---|---|---|
| P4.1 | Audit every write path against the fsync ordering the Spec prescribes | Spec §4.7, HC-4, FR-12 |
| P4.2 | Crash tests that kill a real process mid-operation. No indirection layer inside `veil-core` — that would put a seam in shipped code to serve a test | Spec §9, HC-4 |
| P4.3 | Compaction: select by garbage ratio, copy live extents, single generation step, remove old pack | Spec §4.5, FR-23, FR-24, FR-25 |
| P4.4 | **Nothing at open**: no write, no walk of the packs. Bytes an interrupted operation left are found by reclaiming and by reporting the figures, both of which the user asks for; read-only vaults open read-only and say so at open | Spec §4.5, FR-8, FR-22, FR-26, FR-27, HC-4 |
| P4.5 | Missing-but-referenced pack treated as total damage to that pack — vault opens, affected entries enumerated | Spec §4.5, S-4 |
| P4.6 | Crash suite green across add, replace, delete, and compact | Spec §9, HC-4 |

**Exit:**
- No interruption at any fsync boundary yields an unopenable vault or loses an entry that existed beforehand — with the limit Spec §9 states: the kill is a process kill, not a power cut.
- Compaction of a vault of any size needs working space bounded by roughly one pack (FR-25) — verified against a vault large enough that the difference is unambiguous.
- An interrupted compaction leaves nothing unreachable, and the space it left is recovered by the next reclaim rather than by anything happening at open (FR-8, FR-23).
- Opening a vault writes nothing and measures nothing, with the generation unchanged — the property FR-27 depends on (FR-22, S-2).
- A vault on read-only media opens, and says so at open rather than at the first failed write (FR-26).

---

## Phase 5 — Portability by Construction (Spec M5)

*Proves the half of HC-8 that one machine can prove: that no host fact reaches the stored format. The other half needs a second machine and is Phase 8.*

**Entry:** Phase 4 exit met.

| Task | Work | Cites |
|---|---|---|
| P5.1 | NFC normalisation on ingest; exact case-sensitive comparison thereafter | Spec §4.6, HC-8, FR-13 |
| P5.2 | Extraction representability check — stop and ask rather than silently altering a name, including the names reserved on a platform this machine is not | Spec §4.6, FR-31, HC-8 |
| P5.3 | Build the portability **fixture** and the comparison it feeds: vaults carrying Latin, Thai, Arabic, Han and emoji names, NFC/NFD pairs, and Windows-reserved names, generated and committed, with the byte-for-byte check written and passing against them here. Phase 8 runs it elsewhere; nothing about it is written then | Spec §9, HC-8 |
| P5.4 | Network-path detection and the best-effort locking advisory | Spec §2, FR-26, FR-27 |
| P5.5 | Scale tests marked `#[ignore]` and run on request: a multi-gigabyte entry and a vault at C-1's limit | Spec §9, S-1, S-2, C-1, C-2 |
| P5.6 | Fix the maximum path-metadata length, resolving that Spec open item | FR-10, Spec §11.1 |
| ~~P5.7~~ | **Done ahead of its phase**, during the v2.0 revision, because the format is unreleased and the window closes at 2.0.0: `next_pack_id` stored in the index, allocation taken from it, and a case asserting that emptying a vault and reclaiming does not hand a spent identifier back | Spec §4.3, §4.5, HC-3, FR-2 |

**Exit:** a vault written here carries no fact about this machine — normalisation, separators, case and reserved names all handled at the boundary, asserted by test rather than by argument; the fixture and its comparison exist and pass locally; peak memory does not scale with file size (S-1) and open time does not scale with vault size (S-2), both asserted rather than assumed; and reclaiming the highest pack does not reissue its identifier (P5.7).

**P5.7 was done early and is left in the list rather than deleted from it.** Specification §4.3 defines the field, and every vault in existence was written by its author on this machine — so adding it cost a line, while adding it after 2.0 would cost a format version and a migration path Requirements §2.2 has not built. The row stays so the phase's record shows what it contained, struck through rather than removed.

---

## Phase 6 — GUI Foundation (Spec M6)

*Proves the webview cannot leak the index, and that the one thing the interface exists to do — display the user's own filenames correctly — actually works.*

**Entry:** Phase 5 exit met.

| Task | Work | Cites |
|---|---|---|
| P6.1 | Tauri v2 shell over `veil-core`; operations on a worker thread, progress marshalled to the UI thread | Spec §5.3, A-3, A-4 |
| P6.2 | Ephemeral webview storage configured per platform — all three written, none left to default, whichever one is running | Spec §5.3, HC-1 |
| P6.3 | CSP restricted to the bundled origin; no `localStorage`, `sessionStorage`, or IndexedDB; devtools compiled out of release | Spec §5.3, HC-1 |
| P6.4 | **Webview persistence test** — marker filenames, browse, close, then search webview data, caches, and temp directories. Written once and run on macOS here; Phase 8 runs the same test on each platform before it ships, and no platform ships without it | Spec §9, §5.3, HC-1 |
| P6.5 | Virtualised entry list at the density and typography the design fixes, including tabular numerals | Design §2.3, §3.2 |
| P6.6 | Complex-script rendering verified in both themes — the evidence that decided the toolkit | HC-8, Design §2.2 |
| P6.7 | Whole-window drop target naming the count before release; native file dialogs | Design §3.3, FR-9, FR-16 |

**Exit:** the persistence test is green on macOS — any marker found is an HC-1 defect and blocks the phase, and the same test blocks each later platform in Phase 8. Thai, Arabic, Han and emoji filenames render correctly in light and dark. Dropping 34 files shows "34" before release.

---

## Phase 7 — GUI v1 (Spec M7)

*Proves the product.*

**Entry:** Phase 6 exit met.

| Task | Work | Cites |
|---|---|---|
| P7.1 | Unlock screen — four elements only, a visibly alive working state during derivation, wrong password and damaged vault as distinct outcomes | Design §5, FR-2, C-3 |
| P7.2 | Superseded and too-new format messages | Design §5, FR-5, FR-30 |
| P7.3 | Vault creation: password subject to C-4, the unrecoverability block, explicit acknowledgement rather than a pre-ticked box | Design §8.2, HC-7, C-4, FR-1, FR-29 |
| P7.4 | Identity bar with lock state legible at a glance, and the statistics line | Design §3.2, FR-8 |
| P7.5 | Search and the folder-grouping view toggle — a view control, not a tree | Design §3.2, FR-7 |
| P7.6 | Add flow with progress and cancel, and the retained-originals clause on completion | Design §8.3, FR-9, FR-14, FR-29 |
| P7.7 | Extract flow: destination always chosen, overwrite confirmed by name, the unprotected-copy line every time | Design §6, FR-16, FR-18, FR-29 |
| P7.8 | Unrepresentable-name prompt at extraction | Design §6, FR-31 |
| P7.9 | Delete with the persistence clause, and reclaim space with the figures in the button | Design §8.4, FR-21, FR-22, FR-23, FR-8, FR-29 |
| P7.10 | Lock action and a locked screen distinct from a greyed-out list | Design §8.5, FR-3, HC-1 |
| P7.11 | Three-part error presentation — what happened, what state things are in, what you can do | Design §4.2 |
| P7.12 | Constrained conditions: vault in use, changed on disk, storage gone, destination full, limits exceeded, damaged region marked per-entry | Design §4.3, FR-15, FR-26, FR-27, FR-28, S-4 |
| P7.13 | Damage check: time estimate before starting, per-entry progress, cancellation returning partial results, and a result that names failing files and states plainly that Veil cannot recover them | Design §8.6, FR-33, S-4 |
| P7.14 | Vocabulary audit against the Design §7 table across GUI and CLI alike | Design §7 |
| P7.15 | Packaging for macOS: bundle UTI, signing, notarisation. The Windows and Linux artifacts are Phase 8 | Spec §8.2 |
| P7.16 | The release states the platforms it was run on, and says "coming soon" about the others rather than offering them | Requirements §2.1, §8 |

**Exit:** every functional requirement is reachable from the GUI; the vocabulary audit is clean in both applications; the macOS package installs, opens a vault, and is the 2.0.0 release.

---

## Phase 8 — Windows, then Linux (Spec M8)

*Proves HC-8 in the direction only a second machine can, and closes the deferral Requirements §2.2 records.*

**Entry:** Phase 7 exit met, 2.0.0 shipped for macOS, and a machine of the target platform available. **This phase is blocked by hardware rather than by work**, which is why it is last and why nothing before it waits on it.

Run once per platform, in this order, and the platform ships at the end of it:

| Task | Work | Cites |
|---|---|---|
| P8.1 | Build the workspace and run the whole suite on the target platform. Failures here are ordinary defects, not portability findings, until shown otherwise | Spec §8.1 |
| P8.2 | Run P5.3's fixture in both directions: this platform opens the vaults macOS wrote and compares names and content byte-for-byte, and macOS opens what this platform writes | Spec §9, HC-8 |
| P8.3 | Run the webview persistence test on this platform. **No platform ships without it** — an unverified webview configuration is an unverified HC-1 claim | Spec §5.3, §9, HC-1 |
| P8.4 | Platform-specific paths exercised by hand where no test can reach them: advisory locking on a network share, a read-only mount, directory durability on the platform's own filesystem | Spec §2, §4.5, §4.7, FR-26 |
| P8.5 | Package per Spec §8.2 — Windows installer and association, or Linux AppImage with the WebKitGTK version check — and release with its own version number | Spec §8.2 |

**Exit, per platform:** the suite passes on it, a vault written on macOS opens on it with identical names and content and the reverse holds, the persistence test is green, and the package installs. Only then is the platform announced as supported.

**Why the whole phase is worth keeping written down rather than left as "port it later."** Two things in it are cheaper to know early and are the reason P5.3 and P6.4 are built before the machines exist: the fixture and the persistence test. Both are written on macOS by phases that have to write them anyway, so this phase is an afternoon of running per platform rather than a project — which is the property that makes deferring it honest rather than convenient.

---

## Cross-Cutting Obligations

These apply to every task in every phase and are part of the definition of done, not a final sweep:

- **No plaintext, key material, or password** reaches an error message, a `Debug` output, or a log line (HC-1, HC-2, Spec §6).
- **Every new long-running operation** gets progress reporting and cooperative cancellation when it is written, not afterwards (A-3).
- **Every new error variant** carries the state fact the three-part message needs (Design §4.2, Spec §6).
- **Anything learned that changes HOW** goes into the Technical Specification as a version bump via the feedback protocol (G-11, G-24). This document records sequencing, never design.

---

## Sequencing Risks and Blocked Tasks

**No task is currently blocked.** The three open questions that held up Phase 2 and Phase 3 were resolved in the v1.1 suite: replace matches on full path (FR-13), whole-vault verification is in scope for both applications (FR-33), and compaction gains no scheduling hook (FR-23). Their answers are recorded in the Requirements' resolved ledger.

**Ordering choices worth defending:**

- **P1.11 gates everything.** The corruption suite is not a Phase 1 deliverable to be finished later; it is the condition for starting Phase 2.
- **CLI before durability work.** Crash-injection through a command is cheap; through a GUI it is not.
- **Portability by construction before GUI** (Phase 5), even though the second machine comes after it. The reason the old ordering gave — that a platform-specific bug found after Phase 6 costs three times as much to reproduce — applies to *where the bug is written*, not to where it is observed. Phase 5 is where every host fact is kept out of the format, and it stays ahead of the GUI for exactly that reason. What moved to Phase 8 is the observation, which needs hardware.
- **Argon2id cost measured on the weakest target** (P1.12), not the development machine. A vault that cannot be opened on a modest laptop is a worse failure than a slow derivation on a fast one. Still unmeasured, and now the oldest open item in the plan.

**Standing risk:** the webview configurations of P6.2 fail *silently* when wrong — a running application looks identical whether or not it is writing a cache. P6.4 is the only thing that detects it, which is why it is an exit condition rather than a task, and why P8.3 repeats it per platform rather than trusting the macOS run.

**Standing risk, new at v2.0:** two platforms now go a long time between being written for and being run. The mitigation is not optimism — it is P5.3 and P6.4 being built before the machines exist, so Phase 8 is a run rather than a project, and Spec §8.1's line that writing for one platform is forbidden while verifying on one is accepted. The failure mode to watch for is that line eroding quietly: a `#[cfg(target_os = "macos")]` that should have been a portable path, added because it was the only one anyone could test.

---

## Open Questions

- **~~Whether the scale tests of P5.5 run on developer hardware or a dedicated runner.~~** Resolved: developer hardware on request, since there is no runner (Spec §8.1).
- **Whether Phase 7 ships as one release or the GUI lands incrementally behind a pre-release tag.** Affects nothing technical; affects when the 2.0.0 tag is cut. Resolver: owner, at Phase 6 exit.
- **Whether Windows or Linux comes first in Phase 8.** Depends entirely on which machine appears first, so it is not worth deciding in advance. Resolver: whichever it is.
- **Whether the CLI ships for Windows and Linux ahead of the GUI.** The command line has no webview, so P8.3 does not apply to it and its Phase 8 is materially shorter — P8.1, P8.2 and P8.4 only. Whether that is worth a separate release, or whether a platform arrives all at once, is a product call rather than a technical one. Resolver: owner, at Phase 8 entry.

### Resolved during v2.0

- **~~How Phase 5 exits without three machines.~~** Resolved by splitting it: what one machine can prove stays in Phase 5, what needs a second becomes Phase 8, and Requirements §2.1 makes macOS the 2.0 platform so the split is a scope decision rather than a gap. The previous exit condition — a portability run in every direction between three platforms — could not be performed by anyone, and an unpassable gate in the middle of a plan does not stop work; it teaches everyone to route around a phase boundary.

### Resolved during v1.2

- **Whether per-phase to-do lists are written ahead of each phase or ahead of the whole plan.** Resolved as **two phases ahead**: Phase 0 and Phase 1 are written now, later phases as they approach. Writing all eight now would fix step-level detail against a Specification that Phase 1's measurements are expected to bump, and G-10 puts these documents close to the work for that reason. Phase 1 is the exception that earns early detail — it gates Phase 2 and its corruption suite has to be designed before the code it attacks exists. The four documents are pinned in the header above.
