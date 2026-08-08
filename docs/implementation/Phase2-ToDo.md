# Veil2 — Phase 2 To-Do: Vault Operations

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-08
**Owner:** wattanit
**Foundation and plan versions this list is built against (G-14):**
- Requirements Document **v1.1** — upstream
- Design Guideline **v1.1** — upstream
- Technical Specification **v1.1** — upstream
- Implementation Plan **v1.3** — upstream; this list expands Plan tasks P2.1–P2.13

This document owns the **step-level breakdown of Phase 2**. It defers what to build to the Requirements, how it presents to the Design Guideline, how it is built to the Specification, and phase sequencing to the Implementation Plan. Enumerated checks live in [Phase2-TestCases.md](Phase2-TestCases.md).

**It is not a shadow spec (G-11).** No item below restates a format, a field, an algorithm, or a parameter value; each names an action and cites the section that defines what the action must produce. Candidates for Specification change are recorded under *Notes for Upstream* and decided by the owner.

---

## Conventions

**Item identifiers** are `P<phase>.<task>.<letter>`, subdividing the Implementation Plan's task numbers. They are section-numbering references, not foundation identifiers (G-19).

**Done** for an item means the cited behaviour is observable, the test cases listed against it pass on all three platforms, and the Plan's standing definition of done holds.

---

## What Phase 2 is for

Phase 1 proved the format. Phase 2 proves the **API**, and the Plan's framing is exact: *sufficient for both frontends, before either exists*. Every shape decided here is inherited by the CLI in Phase 3 and by the GUI in Phase 7, and the two most expensive things to retrofit — progress reporting and cooperative cancellation (A-3) — are precisely the two that a core built without a caller tends to omit. They are therefore P2.2, ahead of the operations that use them, not appended after.

**The exit conditions are behavioural, not structural.** "The full lifecycle runs with no terminal present" is A-1 made observable; it is the single sentence that separates this rebuild from the original, whose logic could not be exercised without a pseudo-terminal. Nothing in this phase may require a TTY, a prompt, or a process boundary to test.

**What Phase 2 deliberately does not do.** Compaction (FR-23, FR-24) is Phase 4. Reconciliation of orphaned packs at open (FR-32) is Phase 4. NFC normalisation of names (§4.6) is P5.1 — Phase 2 matches on the stored form exactly, which is the *rule* §4.6 fixes; the *normalisation* that makes the rule portable arrives in Phase 5. Crash testing (Spec §9) is P4.2: Phase 2 writes in the right order, Phase 4 kills a process to see whether that held. Each of these is named here so that "Phase 2 is done" cannot be read as "these were considered and found unnecessary".

---

## P2.1 — Create, open, lock, and the advisory lock

*Plan P2.1 · Spec §2, §5.1 · FR-1, FR-2, FR-3, FR-26, A-7*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P2.1.a | An advisory lock taken on a lock file at open and held for the lifetime of the open vault | FR-26, Spec §2 | T2.1 |
| P2.1.b | A second opener told the vault is in use, by name, rather than being allowed to write | FR-26, Spec §6 | T2.1 |
| P2.1.c | The lock released when the vault is closed, including on an error path and on unwind | FR-26, HC-4 | T2.2 |
| P2.1.d | `lock` consuming the vault and zeroising every key it holds, so a locked vault is not a vault with a flag set | FR-3, HC-2, Spec §5.1 | T2.3 |
| P2.1.e | A write refused when the index generation on disk is ahead of the one held in memory | FR-27, Spec §4.3, §4.4 | T2.5 |
| P2.1.f | Nothing in the open path stored in a process-global, so two vaults may be open in one process | A-7, Spec §2 | T2.4 |
| P2.1.g | A reload that adopts an external change using the keys already held, so recovering from P2.1.e does not cost the password again | FR-27 | T2.41 |

**Why P2.1.c is its own item.** A lock held for "the lifetime of the open vault" is trivially true on the success path and is exactly what a `Drop` implementation is for. The case that decides whether FR-26 holds is the one where `add` fails halfway: if the lock leaks, the user's next attempt reports their own vault as in use, and the remedy — delete a file you were never told about — is worse than the fault.

**Why P2.1.e is here rather than in Phase 5.** The Plan attaches FR-27 to P1.8 (the generation counter exists) and to P5.4 (the network-path advisory). Neither is the check itself. The counter is only a detector if something consults it before writing, and the only phase that introduces writes is this one. See *Notes for Upstream*, item 1.

---

## P2.1a — The index at open

*Plan P2.1a · Spec §4.3, §5.1 · FR-6, FR-22, S-2*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P2.1a.a | The whole index decrypted at open and held in memory; browsing thereafter reads no file | FR-6, S-2 | T2.4 |
| P2.1a.b | Statistics served from the index document, never computed by scanning packs | FR-8, FR-22 | T2.27 |
| P2.1a.c | Open touching no pack file, so open cost tracks entry count and not vault size | S-2, FR-22 | T2.4 |

**P2.1a.c is asserted by removing the packs, not by timing.** A timing assertion on a machine doing other work is a flake generator. What S-2 states is not that open is fast but that vault size is not an input to it — and a vault whose pack files are gone entirely still opening, enumerating every entry, and reporting its statistics is that property in a form nothing which read a pack could survive.

---

## P2.2 — Progress and cancellation

*Plan P2.2 · Spec §2 · A-3, FR-14, FR-19*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P2.2.a | A progress sink passed as a parameter, never global, with a no-op implementation the CLI can pass | A-3, Spec §2 | T2.6 |
| P2.2.b | A cancellation token passed as a parameter, shareable across threads so a UI thread can set it | A-3, Spec §2 | T2.7 |
| P2.2.c | Cancellation checked at chunk boundaries, bounding latency to a constant number of chunks rather than to the file | Spec §2, FR-14 | T2.8 |
| P2.2.d | Progress reported for every long operation: ingest, extraction, folder ingest, and verification, each in the unit that suits it | FR-14, FR-19, Spec §4.8 | T2.6, T2.34 |
| P2.2.e | Cancellation reported as its own outcome carrying whether the operation rolled back, never as a generic failure | FR-14, Spec §6 | T2.7 |

**On P2.2.e.** A cancelled operation and a failed one lead the user to different places: one is a decision they made, the other is a problem they must act on. The Design Guideline promises a cancelled ingest leaves the vault as though it never began; a caller that cannot tell cancellation from failure cannot make that promise.

**On the unit in P2.2.d.** A folder ingest is one operation to the person watching it, so it counts entries. Forwarding each file's byte counter would send the figure back to zero at every file, and a bar that restarts is worse than no bar. This is the same choice §4.8 makes for verification, for the same reason. A caller wanting byte-level progress for one file drives the single-file path itself.

---

## P2.3 — Ingest

*Plan P2.3 · Spec §4.7 · FR-9, FR-12, FR-14*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P2.3.a | The source opened read-only, never modified, moved, or unlinked; nothing in `veil-core` deletes a file outside a vault | FR-9, Spec §4.7 | T2.10 |
| P2.3.b | Pack data written and fsynced before the index generation that references it advances | FR-12, Spec §4.7 | T2.11 |
| P2.3.c | Success reported only after the index write returns, never before | FR-12 | T2.11 |
| P2.3.d | A cancelled or failed ingest advancing no generation and returning the packs to what they held | FR-14, Spec §4.7 | T2.7 |
| P2.3.e | Entry identifiers never reused, including after delete, after emptying the vault, and across a reopen — the counter stored rather than derived | Spec §3.2, §4.3 | T2.25 |
| P2.3.f | The full path — folder and name together — as the entry's identity, compared exactly | FR-13, Spec §4.6 | T2.20 |

**P2.3.e is a cryptographic requirement wearing bookkeeping clothes.** The entry identifier is bound into the DEK-wrapping nonce and into the content AAD. Reusing an identifier after a delete would let a wrapped key from the dead entry decrypt under a live one's nonce. The monotonic counter is not tidiness.

**It is also the one place this phase changed the format.** Deriving the next identifier from the highest *live* entry — the obvious implementation, and the one that was there — reissues an identifier the moment the highest entry is deleted. The counter has to outlive the entries it counted, so it is stored: the index document carries `next_entry_id`. Recorded in *Notes for Upstream*, item 5.

---

## P2.4 — Folder ingest

*Plan P2.4 · Spec §4.7 · FR-10, FR-11*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P2.4.a | A walk over regular files only, storing each with its path relative to the added root as folder metadata | FR-10, FR-7 | T2.13 |
| P2.4.b | Symbolic links not followed, at every level — the directories walked as well as the files stored | FR-11 | T2.14 |
| P2.4.c | Each skipped link recorded and returned to the caller, not merely omitted | FR-11 | T2.14 |
| P2.4.d | A walk that terminates on a tree containing a link cycle | FR-11 | T2.15 |
| P2.4.e | Path separators normalised to `/` in the stored folder field regardless of host | Spec §4.6, HC-8 | T2.13 |

**P2.4.c is the difference between a requirement and a side effect.** Not following a link and not mentioning it produces a vault that is silently missing files the user believes they added. FR-11 says *recorded as skipped*; a return value the caller can present is the only form of "recorded" that survives into the two frontends.

---

## P2.5 — Extraction

*Plan P2.5 · Spec §4.7 · FR-16, FR-17, FR-19, FR-20*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P2.5.a | Output written to a caller-supplied `Write`, so no path is chosen inside the core | FR-16, S-1, HC-2 | T2.19 |
| P2.5.b | The content hash compared after the final chunk, and failure named with the entry | FR-17, S-4 | T2.17 |
| P2.5.c | Partial output removed on any failure, so nothing is left looking like a valid file | FR-17, HC-3 | T2.17 |
| P2.5.d | Partial output removed on cancellation as well as on failure | FR-19, FR-17 | T2.9 |
| P2.5.e | Peak memory independent of entry size in both directions | FR-20, S-1 | T2.18 |

**Removal is the caller's act, and the core must make it possible.** `veil-core` writes to a `Write` and therefore cannot delete a file it never named — that is P2.5.a and it is deliberate (HC-2, and the original Veil's habit of writing into the working directory over the user's original). So P2.5.c and P2.5.d are satisfied *in the API's shape*: extraction to a path is a thin caller-side wrapper that owns the file it created and removes it, and that wrapper lives here rather than being written twice in Phases 3 and 7.

---

## P2.6 — Replace

*Plan P2.6 · Spec §4.6, §4.7 · FR-13, HC-4*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P2.6.a | The target matched on folder and name together, so a same-named file in another folder never matches | FR-13, Spec §4.6 | T2.20 |
| P2.6.b | New content written and durable before any index generation advances | FR-13, HC-4, Spec §4.7 | T2.21 |
| P2.6.c | One generation step that simultaneously points the path at the new entry and marks the old extents reclaimable | FR-13, FR-21 | T2.22 |
| P2.6.d | A replace whose ingest fails leaving the previous entry intact and reachable | HC-4, FR-13 | T2.21 |

**There is no window in which zero intact versions exist**, and P2.6.c is where that is either true or false. Two generation steps — remove then add — would create one, and it would be invisible in every test that does not crash between them.

---

## P2.7 — Delete

*Plan P2.7 · Spec §4.5 · FR-21*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P2.7.a | The entry removed from the index and immediately unreachable through the API | FR-21 | T2.23 |
| P2.7.b | The summed length of the entry's extents added to reclaimable bytes | FR-21, FR-8 | T2.24 |
| P2.7.c | Stored bytes left in place until compaction, with nothing in the core implying otherwise | FR-21, FR-29 | T2.24 |

**P2.7.c is a documentation obligation on the core, not just on the frontends.** FR-29 requires the product to say that deleted bytes remain. The frontends say it to the user; the core's own API documentation must say it to the next implementer, or a later contributor will add the "obvious" truncation and turn a bounded honesty problem into a durability bug.

---

## P2.8 — Statistics

*Plan P2.8 · Spec §4.3 · FR-8, FR-22*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P2.8.a | Each of the four totals updated incrementally by every mutating operation | FR-22, Spec §4.3 | T2.26 |
| P2.8.b | No code path that derives a total by scanning packs or entries at read time | FR-22, S-2 | T2.27 |
| P2.8.c | A recount function used by tests only, never by the API, as the oracle the incremental path is checked against | FR-8 | T2.26 |

**P2.8.c exists because incremental accounting is the classic place for slow divergence.** A total that drifts by one delete in a hundred is invisible until a user runs compaction on a figure that was wrong. The oracle is cheap; without it, T2.26 would be asserting the implementation against itself.

---

## P2.9 — Limits

*Plan P2.9 · FR-15, C-1, C-2*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P2.9.a | An addition beyond the entry limit refused, naming the limit and the current value | FR-15, C-1 | T2.28 |
| P2.9.b | An entry beyond the file-size limit refused, naming the limit and the actual size | FR-15, C-2 | T2.29 |
| P2.9.c | The size limit enforced during the stream, not only from a stated length, so a source that lies about its size is still refused | FR-15, C-2 | T2.29 |
| P2.9.d | A refused addition advancing no generation, leaving the vault unchanged | FR-15, HC-4 | T2.28, T2.29 |

**P2.9.c is the item that matters.** A limit checked from `metadata().len()` before the copy is a limit on files, not on content: a growing file, a pipe, or any `Read` that is not a file passes the check and then writes past the bound. The stream is the only place the actual byte count is known.

---

## P2.10 — Password change

*Plan P2.10 · Spec §3.1 · FR-4*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P2.10.a | A new KEK derived from the new password with a fresh salt, and the master key rewrapped under it | FR-4, Spec §3.1 | T2.30 |
| P2.10.b | No content, no index, and no entry key touched — the wrapped master key and the header fields around it are the whole change | FR-4, A-6 | T2.30, T2.31 |
| P2.10.c | The old password verified before anything is written | FR-4, FR-2 | T2.33 |
| P2.10.d | A failed or interrupted change leaving the vault openable with the old password | HC-4, FR-4 | T2.33 |

**FR-4's acceptance standard is a time bound, and P2.10.b is how it is met** rather than measured: if nothing but the header is rewritten, size-independence is structural. T2.31 asserts the structure — one file written, of fixed length — not a stopwatch.

---

## P2.11 — Integration tests

*Plan P2.11 · Spec §9 · A-1*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P2.11.a | The full lifecycle driven through the public API with no process, no terminal, and no prompt | A-1, Spec §9 | T2.34 |
| P2.11.b | Every operation exercised through the same API a frontend would use, never through a crate-private path | A-1, A-4 | T2.34 |

**This is the exit condition of the phase in test form.** The original Veil's logic could not be exercised without a pseudo-terminal, which is why it had fourteen tests. If any Phase 2 behaviour needs a terminal to observe, the API is wrong and the fix is in the API.

---

## P2.12 — Property tests

*Plan P2.12 · Spec §9*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P2.12.a | Arbitrary byte sequences at arbitrary lengths, including zero, surviving ingest and extraction byte-identically | Spec §9, FR-16 | T2.35 |
| P2.12.b | Arbitrary sequences of add, replace, and delete leaving statistics equal to a full recount | FR-22, Spec §9 | T2.36 |
| P2.12.c | Lengths spanning the chunk boundary explicitly, since off-by-one at the boundary is the defect class this is for | Spec §9 | T2.35 |

**P2.12.c is stated because random lengths rarely land on a boundary.** A generator over 0..1 MiB hits the exact chunk length with probability one in a million; the case that breaks a lookahead implementation is exactly there, so it is generated deliberately rather than hoped for.

---

## P2.13 — Whole-vault verification

*Plan P2.13 · Spec §4.8 · FR-33, S-4*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P2.13.a | Verification reusing the extraction path with output discarded, so it cannot diverge from what extraction checks | FR-33, Spec §4.8 | T2.37 |
| P2.13.b | Nothing written during verification, and no exclusive lock required | Spec §4.8 | T2.40 |
| P2.13.c | A failing entry recorded and verification continuing, returning every failure by name | FR-33, S-4 | T2.38 |
| P2.13.d | Progress reported per entry rather than per byte | Spec §4.8, Design §8.6 | T2.6 |
| P2.13.e | Cancellation returning the entries verified so far and their results | Spec §4.8, FR-14 | T2.39 |
| P2.13.f | Never scheduled, never automatic, never triggered at open | FR-33, FR-23 | T2.4 |

**P2.13.a is the whole design of §4.8.** A verification routine that re-implements the read path verifies its own re-implementation. Reuse is what makes "verification passed" mean "extraction will succeed".

---

## Exit

The Implementation Plan's Phase 2 exit conditions govern. Restated as the checklist to run:

- The full lifecycle runs with no terminal present (T2.34) — A-1 made observable.
- Statistics match a full recount after an arbitrary sequence of add, replace, and delete (T2.26, T2.36).
- A cancelled ingest leaves a vault indistinguishable from one where it never began (T2.7).
- Password change completes in time independent of vault size (T2.30, T2.31).

**One item is met in part and is recorded as such.** T2.11 asserts that the index never names bytes outside a pack, which is the observable half of FR-12; the fsync *ordering* is unasserted until the filesystem seam of P4.2 exists. Nothing in this phase claims otherwise, and the exit conditions above do not depend on it.

---

## Notes for Upstream

Recorded per G-24, decided by the owner, absorbed as Specification or Plan bumps or dropped. Nothing below is decided by this document. **Items 2 through 8 were absorbed into Specification v1.2; item 1 into Plan v1.4.**

**1. FR-27's write-time check is not scheduled by the Plan.** The Plan attaches FR-27 to P1.8 (the generation counter) and P5.4 (the network-path advisory). The counter is a detector only if a write consults it, and Phase 2 is the phase that introduces writes, so the check is implemented here as P2.1.e. Either the Plan gains an explicit task or P2.1 gains FR-27 in its citation list. Resolver: owner, at the next Plan bump.

**2. `proptest` is named in Spec §9 but absent from the §7 dependency table.** §7 calls itself a locked initial set with an acceptance policy; a dev-dependency that §9 mandates should appear there, or §7 should say the table covers runtime dependencies only. Resolver: owner, at the next Specification bump.

**3. §4.8 requires verification to run on a read-only vault, which constrains FR-26's lock.** An exclusive advisory lock taken unconditionally at open makes a read-only vault unopenable, and §4.5 already requires read-only media to open. The implementation falls back to opening without a lock when the lock file cannot be created or when the filesystem refuses the lock outright, and reports the vault as read-only; only genuine contention is `VaultInUse`. §2 says the lock is held "for the lifetime of the open vault" without saying what happens when it cannot be taken for reasons other than contention — and telling a user their vault is "in use" because their filesystem does not implement locking sends them hunting for a second window that does not exist. Resolver: owner.

**4. Extraction-to-a-path is an API surface §5.1 does not name.** §5.1 gives `extract` a `Write`, which is what makes S-1 structural, but FR-17's "incomplete output is removed" can only be honoured by whoever owns the file. Implementing that wrapper twice — once in the CLI, once in the GUI — is how the two frontends come to differ, which A-4 forbids. It is implemented once in `veil-core` as P2.5.c. Resolver: owner, as a §5.1 addition.

**5. The index document gained a `next_entry_id` field (P2.3.e).** §4.3's model lists no counter, and the identifier was therefore derived from the highest live entry — which reissues a deleted entry's identifier, and the identifier is bound into the DEK-wrapping nonce and the content associated data (§3.2, §3.3). This is a format change, made because the alternative is a nonce-reuse defect no functional test would surface. It is additive and CBOR-tolerant, so a reader without it is unaffected, and no format version has been released. §4.3's model should name it. Resolver: owner, at the next Specification bump.

**6. `Vault::reload` is an API surface §5.1 does not name (P2.1.g).** FR-27 requires the product to detect a change, refuse to write over it, *and offer to reload*. §5.1's signature list stops at the refusal. Requiring the password again to get past it would make the safe answer cost more than the unsafe one. Resolver: owner, as a §5.1 addition.

**7. A read-only vault refuses writes as an I/O failure, because §6 has no variant for it.** §4.5 and §4.8 both require read-only vaults to open, so the refusal exists; the taxonomy's nearest fit is `Io` carrying a read-only kind, which is accurate but gives the frontends nothing to key a message on. Either §6 gains a variant or the Design Guideline's constrained-conditions table (P7.12) reads the kind. Resolver: owner.

**8. `proptest` and the pack-cap precedent both argue that C-1 and C-2 belong in the API.** Limits are implemented as a `Limits` value defaulting to C-1 and C-2, following P1.9.e's precedent for the pack cap, because FR-15's requirement is that the refusal *names both numbers* — and a refusal only reachable by writing 64 GiB is a refusal nobody has watched fire. Recorded rather than assumed. Resolver: owner.

---

## Open Questions

- **~~Whether the fsync ordering itself gets asserted before Phase 4.~~** Resolved: no indirection layer. Phase 4 kills a real process, or the ordering stays unverified. See Spec §11.1.
- **Carried from Phase 1:** the Argon2id measurement against C-3 — the working values are chosen, nothing is measured. `cargo-fuzz` is declined (Spec §11.1).

---

## Resolved since drafting

- **~~Whether a cancelled ingest should leave its unreferenced pack bytes or truncate them.~~** Truncated, via a rollback on the pack sink. §4.7's "indistinguishable from one where the operation never started" then holds byte-for-byte rather than only in the index, and the statistics oracle of P2.8.c becomes exact. Safe because packs are append-only and the exclusive lock means the bytes above the starting offset are the operation's own. Reconciliation (FR-32) is still needed for the orphans a *crash* leaves, where no rollback ran — this narrows the window rather than closing it.
- **~~What the lock file is called and whether it is inside the vault directory.~~** `veil.lock`, inside. A copy of the directory stays a copy of the vault; a sync tool replicating the file is harmless because the lock lives in the OS rather than in the file's contents. Recorded as a §4.1 addition in *Notes for Upstream*.
- **~~Whether `delete` of the last entry should reset the entry-identifier counter.~~** It does not, and the counter is stored rather than derived. See P2.3.e and upstream note 5.
