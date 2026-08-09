# Veil2 — Phase 5 To-Do: Portability by Construction

**Version:** 1.0
**Status:** draft
**Date:** 2026-08-09
**Owner:** wattanit
**Foundation and plan versions this list is built against (G-14):**
- Requirements Document **v2.0** — upstream
- Design Guideline **v2.0** — upstream
- Technical Specification **v2.0** — upstream
- Implementation Plan **v2.0** — upstream; this list expands Plan tasks P5.1–P5.6 (**P5.7 is not repeated here** — the Plan records it as already built, ahead of this phase, during the v2.0 revision)

This document owns the **step-level breakdown of Phase 5**. It defers what to build to the Requirements, how it presents to the Design Guideline, how it is built to the Specification, and phase sequencing to the Implementation Plan. Enumerated checks live in [Phase5-TestCases.md](Phase5-TestCases.md).

**It is not a shadow spec (G-11).** No item below restates a format, an algorithm, or a parameter value that the foundation already fixes. Two of this phase's tasks — P5.2 and P5.6 — resolve items the foundation deliberately left open (Requirements §9; Spec §4.6's representability check names the rule but not the exact set it covers). Where that happens, the concrete reading this phase implements is recorded under *Notes for Upstream* rather than written here as if it had always been the standard.

---

## Conventions

**Item identifiers** are `P<phase>.<task>.<letter>`, subdividing the Implementation Plan's task numbers. They are section-numbering references, not foundation identifiers (G-19).

**Done** for an item means the cited behaviour is observable, the test cases listed against it pass, and the Plan's standing definition of done holds.

---

## What Phase 5 is for

HC-8 makes portability defect-grade: "a vault written on one platform opens on the others with identical contents and identical file names, without conversion." Proving that needs two things — that no host fact reaches the stored format, and that a second machine confirms it opens what the first wrote. Only the first needs a machine that exists today. The Plan splits the requirement along exactly that line: what one machine can prove stays here; what needs a second machine is Phase 8, run once macOS 2.0.0 has shipped and a Windows or Linux machine is available.

**This phase touches no user interface.** Phases 1–4 built and hardened `veil-core` and the CLI that drives it; Phase 6 is where the GUI starts. Everything below is core behaviour plus the CLI surface Phase 3 already established — a new note in `announce()`, a new exit code, no new screen.

**What this phase deliberately does not do.** No GUI prompt for an unrepresentable name — that is Design §6's interactive "ask for an alternative," and it needs a window to ask in (P7.8). This phase builds the check the prompt will call; a non-interactive caller (a script, or the CLI today) gets a clear refusal instead of a question. No folder-preserving "extract everything" command — that does not exist yet in any phase's plan, and P5.2's check is written so it applies the moment one does. No change to locking's mechanism — P5.4 adds a fact about *where* a vault lives, not a different locking strategy.

---

## P5.1 — Name normalisation

*Plan P5.1 · Spec §4.6 · HC-8, FR-13*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P5.1.a | `name` and `folder` normalised to Unicode NFC at every point a caller supplies one: the top of `Vault::add`, `Vault::replace`, and `Vault::find` — the three functions that currently compare or store these strings as given | Spec §4.6, HC-8 | T5.1, T5.2 |
| P5.1.b | Folder walk (`walk::walk`) normalised the same way, so a tree read through macOS's NFD-yielding filesystem APIs is already NFC by the time it reaches `add` | Spec §4.6, FR-10 | T5.1, T5.4 |
| P5.1.c | Comparison stays exact-byte equality on the already-normalised form (`e.folder == folder && e.name == name`, unchanged) — normalisation is the only new step, never a fuzzier comparison | Spec §4.6 | T5.2, T5.3 |
| P5.1.d | `unicode-normalization` added to `veil-core`'s dependencies, gated by the same `cargo deny`/`cargo audit` run as everything else | Spec §7 | — |

**Why three call sites and not one.** `add`, `replace`, and `find` each take `folder`/`name` from a caller and each compares or stores them independently — `replace` does not call `find`, it repeats the same comparison inline (Spec §4.5's single-generation-step requirement is what keeps them separate: `replace` has to hold the position it found while it stages new content, which `find`'s borrow cannot survive). Normalising once, in a shared helper each of the three calls at its own top, is smaller than restructuring three functions around a fourth.

**Why the walk is a second site rather than the only one.** `walk::walk` produces `folder`/`name` for `add_folder`, but `add_path` derives `name` from `Path::file_name` directly, and a caller can also drive `add` itself with an arbitrary string. Normalising inside `add` catches every caller; normalising inside `walk` as well means the walk's own output (`Found`) is already correct if anything ever reads it before calling `add` — cheap, and consistent with the rest of the module already doing its own path-shaped work (`relative_folder`'s `/`-joining) rather than leaving it to the caller.

---

## P5.2 — Extraction representability

*Plan P5.2 · Spec §4.6 · FR-31, HC-8*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P5.2.a | A pure, platform-dispatched check — "would this name be legal as a filename on the platform this binary is running on" — added to `veil-core`, taking a name and returning why it fails rather than only that it does | Spec §4.6, FR-31 | T5.5–T5.8 |
| P5.2.b | A new `Unrepresentable` reason enum, alongside `Limit` and `Damaged` in `error.rs`: a reserved device name (`CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`), a reserved or control character, a trailing dot or space, or a name that collides with another only by case | Spec §6, FR-31 | T5.5–T5.8 |
| P5.2.c | `Error::NameNotRepresentable { id, reason }`, naming the entry and the reason and nothing else — never the entry's own name, which the caller already holds and is the layer allowed to display it (HC-1, matching `NotFound`'s existing convention) | Spec §6, HC-1, FR-31 | T5.5 |
| P5.2.d | The check wired into the one place a stored name currently becomes a filesystem name outside the vault: `save_copy`'s `to.is_dir()` join in `veil-cli/src/run.rs`, run before the join, refusing rather than truncating or substituting | Spec §4.6, §5.2, FR-31 | T5.9, T5.10 |
| P5.2.e | The case-collision check is against the vault's own other entries in the same folder, not against what happens to exist on disk at the destination right now — two vault entries differing only by case are the failure this exists to catch even when nothing has been extracted yet | Spec §4.6, HC-8 | T5.8 |
| P5.2.f | Written for all three platforms' rules (Windows reserved names and characters; a case-insensitive destination filesystem, which macOS's default APFS volume and every Windows volume both are) even though only the macOS-hosted checks run today (Spec §8.1) | Spec §8.1, HC-8 | T5.6, T5.7 |

**Why the check is pure rather than filesystem-probing.** Asking the destination filesystem whether it would accept a name (create a probe file, inspect an error) is unreliable across mount types and leaves a side effect to clean up on every check. The rules FR-31 is protecting against — Windows' reserved names and characters, and case-insensitivity — are static per platform, not per volume, and encoding them as data is what makes P5.3's fixture and P8.2's cross-platform run testable without touching a filesystem that does not exist here.

**Why the entry's own name never appears in the error.** `error.rs`'s existing convention is that structured reasons and identifiers cross the boundary, not the string a caller already has (`Damaged::Pack { id }`, `Error::VerificationFailed { entries }`, `Error::NotFound`'s doc note that a name is the caller's to attach). `NameNotRepresentable` follows it rather than becoming the first exception: the CLI already knows the name it was trying to write, from the vault it already holds open.

**Why this lands on `save_copy` and nowhere else, for now.** It is the only place in the current CLI where the vault's own `name` becomes an on-disk filename without the caller having chosen every character of it (`to.is_dir()` appends `split(file).1`). A future folder-preserving extraction command inherits the same check by construction, because P5.2.a is written against a name, not against this one call site.

---

## P5.3 — The portability fixture

*Plan P5.3 · Spec §9 · HC-8*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P5.3.a | A fixture vault built once and committed under `crates/veil-core/tests/fixtures/portability/`, holding entries whose names cover Latin, Thai, Arabic, Han and emoji scripts, an NFC/NFD pair of the same visible name, and every Windows-reserved name and character P5.2.b enumerates | Spec §9, HC-8 | T5.11 |
| P5.3.b | The comparison the fixture feeds: open it, extract every entry, and compare name and byte-for-byte content against a recorded manifest — written and passing here, on macOS, now | Spec §9, HC-8 | T5.11, T5.12 |
| P5.3.c | The NFC/NFD pair asserts one outcome deliberately: both spellings of the same visible name resolve to **one** entry, because both were NFC-normalised on the way in (P5.1) — the fixture is where "two visually identical names differ in bytes depending on where the vault was written" (Spec §4.6) is checked directly rather than argued | Spec §4.6, HC-8 | T5.12 |
| P5.3.d | The Windows-reserved names are stored in the vault (Spec §4.6 places no restriction on what a vault may *hold*; FR-31 restricts what may be *extracted onto a given platform*) and the fixture's manifest records which of them P5.2.a is expected to refuse here, on macOS, versus which would only be refused on a case-insensitive or Windows destination | Spec §4.6, FR-31 | T5.11, T5.13 |
| P5.3.e | Nothing about the fixture or its comparison is written when Phase 8 runs — P8.2 opens this same fixture on the target platform and runs this same comparison in reverse | Spec §9, HC-8 | — |

**Why the fixture is committed rather than generated per test run.** A fixture generated fresh on each platform proves that platform can read what it just wrote, which is not the claim HC-8 makes. The vault has to be built once, on one machine, and carried unchanged to the other two — which is exactly what P8.2 does with it. Building it in this phase and committing the bytes is what makes that possible without needing Windows or Linux hardware to exist yet.

**Why reserved names are stored at all.** The alternative — refusing to *ingest* a name that some other platform could not extract — would make a vault's contents depend on which platforms its owner might someday use, which is the same defect HC-8 exists to reject, aimed at the opposite end of the pipeline. Spec §4.6 draws the line at extraction on purpose, and the fixture exists to prove the line holds both ways: everything goes in, and P5.2 is what stops a name coming back out onto a platform that cannot hold it.

---

## P5.4 — Network-path advisory

*Plan P5.4 · Spec §2 · FR-26, FR-27*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P5.4.a | A pure, safe (no `unsafe`, per `#![forbid(unsafe_code)]`) best-effort check — "does this vault's directory sit on what looks like network-backed storage" — added to `veil-core`, exposed as a fact on an open `Vault` alongside `access()` | Spec §2, FR-26 | T5.14–T5.16 |
| P5.4.b | macOS implementation reads `mount`'s output (via `std::process::Command`) and matches the vault directory's path against the listed mount points, flagging filesystem types `nfs`, `smbfs`, `afpfs`, and `webdav` | Spec §2, FR-26 | T5.14, T5.15 |
| P5.4.c | Written for the other two platforms per Spec §8.1 even though neither runs today: Linux reads `/proc/mounts` (plain `std::fs::read_to_string`, no process spawn needed); Windows checks for a UNC path prefix, a known and stated partial answer — a *mapped* network drive letter is not caught by it, and the gap is recorded rather than hidden | Spec §8.1, HC-8 | T5.16 |
| P5.4.d | `announce()` in `veil-cli/src/run.rs` gains a note beside the existing read-only one, in the same honesty-clause register as Design §4.3's fixed wording, stated once at open and never repeated per command | Spec §2, Design §4.3, §7 | T5.17 |
| P5.4.e | The check runs once, at open, from the path already in hand — no new filesystem walk, no repeated stat during the session, so it costs nothing S-2 would notice | Spec §2, S-2 | T5.14 |

**Why the mechanism is a subprocess and not a syscall.** `veil-core` forbids `unsafe` code crate-wide (`#![forbid(unsafe_code)]`, `lib.rs`), and the syscall that would answer this directly (`statfs`, whose `f_flags` carries the `MNT_LOCAL` bit BSD and macOS both expose) is FFI. `mount`'s output is the same information already rendered as text by a tool every macOS and Linux install ships, and running it costs one process spawn at open, on the once-per-session path that already tolerates a stat or two. It is why P5.4.c can answer Linux without a syscall at all: `/proc/mounts` is the same information as a plain-text file.

**Why this is advisory and never a refusal.** Spec §2 already states the honesty clause this task exists to surface: locking on a network filesystem is unreliable, and FR-27's generation counter is what actually protects a vault there, not the lock. A vault does not stop opening because it is on a network path — it opens exactly as it would anywhere else, and the note says what the user should already know before two machines write to it at once.

---

## P5.5 — Scale tests

*Plan P5.5 · Spec §9 · S-1, S-2, C-1, C-2*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P5.5.a | A multi-gigabyte single entry added and extracted, with peak process memory sampled through the run | S-1, C-2 | T5.18 |
| P5.5.b | A vault built to C-1's 65,536-entry limit, with open time measured against a small vault's open time | S-2, C-1 | T5.19 |
| P5.5.c | Both marked `#[ignore = "…"]`, following `kdf_cost.rs` and `reclaim.rs`'s existing convention, and run only on request | Spec §8.1 | — |
| P5.5.d | Landed in a new `crates/veil-core/tests/scale.rs`, rather than folded into `content.rs` or `reclaim.rs`'s existing scale case, so `--test scale -- --ignored` runs every deliberately-slow, deliberately-large case in one place | Spec §9 | — |

**Why these two and not more.** S-1 and S-2 are already asserted indirectly by every ordinary test that passes — peak memory not growing with size and open time not growing with entry count are properties the streaming and index designs hold by construction, not behaviour that only shows up at scale. What scale actually exercises is whether that holds at C-1 and C-2's *stated limits*, which nothing below this size can demonstrate is true rather than merely likely.

---

## P5.6 — Path-metadata length limit

*Plan P5.6 · Requirements §9, FR-10*

| Item | Work | Cites | Tests |
|---|---:|---|---|
| P5.6.a | A limit on the combined length of one entry's `folder` and `name`, enforced in `Vault::stage` — the function `add` and `replace` both call — alongside the existing entry-count and file-size checks | FR-10, FR-15 | T5.20, T5.21 |
| P5.6.b | Reuses the existing `Limit` enum and `Error::LimitExceeded` (a new `Limit::PathMetadata` variant), naming both the configured maximum and what the operation would have produced, exactly as C-1 and C-2 already do | Spec §6, FR-15 | T5.20 |
| P5.6.c | Measured in UTF-8 bytes of `folder` plus one separator byte plus `name` — the same unit the format stores them in, so the limit means the same thing the index does | Spec §4.3 | T5.20 |
| P5.6.d | **Proposed value: 4,096 bytes.** Generous enough that no plausible folder depth or filename length under any of P5.3's scripts approaches it, and small enough to bound one entry's descriptive metadata the same deliberate way C-1 bounds the vault and C-2 bounds a file — recorded here as a proposal because Requirements §9 names the Technical Specification, not this document, as the open question's resolver | Requirements §9 | T5.20 |

**Why this needed a task at all.** Nothing before this phase enforced any limit on `folder` or `name`. An index is rewritten in full on every change (Spec §4.4), so an entry with an unbounded path is the same shape of risk C-1 and C-2 already name for entry count and file size — one caller's pathological input costs every later open and write, not just its own. FR-15 already requires that whatever limit exists names itself and the actual value in the same sentence; this task is where that requirement gets something to apply to.

**Why 4,096 and not a smaller, tighter number.** The obvious alternative is something nearer typical filesystem limits (Windows' historical 260-character `MAX_PATH`, or a per-*segment* cap matching what one directory entry can hold). Both would be answering a different question: FR-31 and P5.2 already handle what a given destination platform can accept at extraction time, per name, per character. This limit is answering "how much descriptive metadata is reasonable to carry for one entry, ever," which is a vault-format question independent of any extraction target — closer in kind to C-1 and C-2 than to a filesystem's own limits. 4,096 bytes holds thousands of characters of Han, Thai, or Arabic even under UTF-8's multi-byte encoding, which a character-counted limit tuned to Latin text would not.

---

## Exit

The Plan's exit conditions for this phase, and where each is met:

- **A vault written here carries no fact about this machine** — normalisation (T5.1–T5.4), the representability check (T5.5–T5.10), and the path-metadata limit (T5.20–T5.21) are each asserted by a test that fails if the property does not hold, not left to argument.
- **The fixture and its comparison exist and pass locally** — T5.11–T5.13, built and committed here for P8.2 to run elsewhere.
- **Peak memory does not scale with file size (S-1) and open time does not scale with vault size (S-2), both asserted rather than assumed** — T5.18, T5.19.
- **Reclaiming the highest pack does not reissue its identifier (P5.7)** — already met; built during the v2.0 revision, ahead of this phase, and not repeated here.

---

## Notes for Upstream

Recorded per G-24. Proposed with the reading this phase implements, so the work is not blocked; the owner rules on each without any code needing to change on absorption, per the pattern Phase 4 established.

1. **FR-31's representability rule names the failure mode — a reserved name, a reserved character, a case collision — but not the exact set for either platform.** *Proposed:* the enumeration in P5.2.b (Windows' `CON`/`PRN`/`AUX`/`NUL`/`COM1`–`9`/`LPT1`–`9`, its reserved characters and control characters, trailing dot or space, and case-insensitivity wherever the destination is a case-insensitive volume). Resolver: owner, to land in Spec §4.6 alongside the rule it enumerates.

2. **Requirements §9's open question on the maximum length of path metadata names the Technical Specification as resolver, not this document.** *Proposed:* 4,096 UTF-8 bytes of `folder` plus `name` combined, argued in P5.6's rationale above. This phase implements the proposed value so the enforcement exists and is tested; the value itself is not authoritative until the Specification states it. Resolver: owner, at the next Specification bump.

3. **Spec §2's network-path honesty clause says the product states the condition at open, and does not say how the condition is detected.** *Proposed:* P5.4's mount-table read on macOS and Linux, and the UNC-prefix heuristic on Windows, each stated as best-effort and each acknowledged as incomplete in its own right (a Windows drive letter mapped to a network share is not detected). Resolver: owner, to decide whether Spec §2 should name a mechanism or continue to leave the "how" to whichever release implements it — the same open-ended honesty clause already covers the case where detection is wrong in either direction.

---

## Open Questions

- **Whether P5.2's check belongs in `veil-core` as a public API or stays private to `save_copy`'s call site.** It is written in `veil-core` either way (P5.2.a); what is open is whether it is exported now, ahead of any second caller, or exported when Phase 6 or 7 needs it for the GUI's own extraction flow. Recorded because exporting it early costs nothing and exporting it late costs a version bump if a signature turns out wrong once a second caller exists. Resolver: owner, at implementation time.
- **Whether the fixture's Windows-reserved names should also be exercised for a *macOS* case-insensitive volume (HFS+, still shipping) in addition to the default case-sensitive APFS most development happens on.** P5.2.f already writes the check for a case-insensitive destination; whether P5.3's fixture comparison needs a second run against an HFS+-formatted image to prove it, here, rather than relying on the check's own unit tests, is a coverage judgement rather than a correctness one. Resolver: owner, when P5.3 is implemented.
