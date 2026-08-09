# Veil2 — Phase 4 To-Do: Durability and Compaction

**Version:** 2.0
**Status:** approved
**Date:** 2026-08-09
**Owner:** wattanit
**Foundation and plan versions this list is built against (G-14):**
- Requirements Document **v2.0** — upstream
- Design Guideline **v2.0** — upstream
- Technical Specification **v2.0** — upstream
- Implementation Plan **v2.0** — upstream; this list expands Plan tasks P4.1–P4.6

*Changes since v1.0 (**major**):* approved, re-pinned, and **P4.4 is rewritten rather than re-worded.** This phase implemented reconciliation at open as a report rather than a discard, and proposed that reading upstream. The owner went further: FR-32 is withdrawn, and nothing happens when a vault is opened at all. The space an interrupted operation leaves is found by reclaiming and by reporting the figures, both of which the user asks for. Everything else in this list stands as built.

> **v1.1 of this document, published earlier the same day, is wrong.** It recorded all ten notes as absorbed. Note 7 was not absorbed — the requirement behind it was withdrawn, which is a different outcome and the one worth remembering.

This document owns the **step-level breakdown of Phase 4**. It defers what to build to the Requirements, how it presents to the Design Guideline, how it is built to the Specification, and phase sequencing to the Implementation Plan. Enumerated checks live in [Phase4-TestCases.md](Phase4-TestCases.md).

**It is not a shadow spec (G-11).** No item below restates a format, an algorithm, or a parameter value. Where this phase must fix something the foundation documents leave open — directory-level durability, what happens to bytes an interrupted operation left, what a crash test is killing — it is recorded under *Notes for Upstream* and decided by the owner, not settled here.

---

## Conventions

**Item identifiers** are `P<phase>.<task>.<letter>`, subdividing the Implementation Plan's task numbers. They are section-numbering references, not foundation identifiers (G-19).

**Done** for an item means the cited behaviour is observable, the test cases listed against it pass, and the Plan's standing definition of done holds.

---

## What Phase 4 is for

Phases 1 to 3 built a vault that works when nothing goes wrong. Phase 4 is the phase that assumes something does. It proves HC-4 — that no single interruption leaves a vault that cannot be opened, and none destroys the only copy of data the vault held — and FR-25, that reclaiming space in a 500 GB vault does not need 500 GB of room to do it.

**The Plan put the CLI first for this phase's benefit.** Crash-injection through a command is cheap; through a UI it is not. Every kill in this phase lands on a real process, and Phase 3 is the reason there is one to kill.

**This phase completes the parity claim of A-4.** Phase 3 stated plainly that reclaiming space was not missing but unbuilt: compaction is not in `veil-core` until P4.3, so the command arrives with it. It arrives here. At the end of this phase, no capability of the core is unreachable from the command line.

**The user-facing words remain fixed by Design §7.** The operation this phase builds is called **reclaim space**. It is never compaction, never vacuum, never garbage collection on screen — those are the words in this repository's source and in these documents, and they stop at the process boundary.

**What this phase deliberately does not do.** No automatic reclaiming, no background thread, no threshold at which the product decides for the user — FR-23 forbids all three, and Phase 3 already refused to grow a flag that would let one be wired into cron. No NFC normalisation (P5.1). No network-path advisory (P5.4). No scale runs beyond the one this phase's own exit condition needs; the rest are P5.5.

---

## P4.1 — Write ordering

*Plan P4.1 · Spec §4.7 · HC-4, FR-12*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P4.1.a | Every write path in `veil-core` enumerated, and each one's ordering stated where the write happens rather than in a document | Spec §4.7, FR-12 | T4.1 |
| P4.1.b | A file's containing directory made durable after the file is created, renamed over, or removed, so a durable file is never left with an undurable name | HC-4, FR-12 | T4.1, T4.2 |
| P4.1.c | The two paths this phase adds — the compacted pack and the removal that follows it — written in the same order as ingest: bytes durable, then the generation that names them, then the removal of what the generation orphaned | Spec §4.5, FR-12, FR-24 | T4.19 |
| P4.1.d | No indirection layer, no injectable filesystem, no test hook. The ordering is checked by killing a process, or it is not checked | Spec §9, §11.1 | T4.2–T4.8 |

**Why P4.1.b is an item and not a detail.** `fsync` on a file makes the file's *contents* durable. It does not make the file's *name* durable: the directory entry that gives it that name lives in the parent directory, and until the parent is itself synced a crash can leave a perfectly durable pack file that nothing can find, or a header that has been renamed over on one machine and not on another. Spec §4.7 fixes the ordering between the pack and the index and is silent on the directory, which is how this was missed through three phases. It is recorded as *Notes for Upstream*, item 1.

**Why P4.1.d closes an open item rather than deferring it.** Spec §11.1 records the fsync ordering as unverified, and records that adding an indirection layer so a test could watch was considered and rejected — it puts a seam in shipped code to serve a test. The alternative it names is killing a real process at M4. That is P4.2, and its results are what resolve the item, in one direction or the other.

---

## P4.2 — Crash tests

*Plan P4.2 · Spec §9 · HC-4*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P4.2.a | A real process killed with an uncatchable signal, part-way through an operation that is genuinely in flight | Spec §9, HC-4 | T4.2–T4.5 |
| P4.2.b | The kill triggered by watching the vault's own bytes appear on disk, not by anything the process was built to tell a test | Spec §9, §11.1 | T4.2 |
| P4.2.c | Four invariants asserted after every kill: the vault opens; every file that existed beforehand is still listed; each of those extracts byte-identically; and the arithmetic holds — file count and stored total match a measurement exactly, while the on-disk total may be *lower* than the measurement by exactly the bytes the kill left, which are counted as space to reclaim | HC-4, FR-8, FR-22 | T4.6 |
| P4.2.d | Add, replace and delete killed through the shipped binary; reclaiming space killed through a subject that is not the shipped binary, for the reason below | Spec §9, §4.5 | T4.5 |
| P4.2.e | A deterministic set that runs with the suite, and a repeated randomised sweep marked `#[ignore]` and run on request | Spec §9, §8.1 | T4.7 |
| P4.2.f | The signal is a kill, not an interrupt. An interrupt is cancellation, it is a different guarantee, and T3.18 already covers it | FR-14, HC-4 | T4.2 |

**Why reclaiming space is killed through a different subject.** A crash test has to reach the code it is about. Multi-pack behaviour is where reclaiming space lives, and the pack cap is 1 GiB — Spec §4.5 made the cap a value the API accepts *precisely* so that a test needing multiple packs does not need multiple gigabytes. The command line does not offer it, and must not: a flag whose only purpose is to make a test cheap is a seam in shipped code, which is the thing P4.1.d refuses. So the compaction crash tests drive a small subject binary that links `veil-core`, takes the cap as an argument, and is killed for real. Nothing is simulated, nothing is pretended, and nothing about it reaches a release: it is a test fixture that happens to be a process, which is the only property the crash tests need from it.

**What these tests cannot reach, stated rather than implied.** Killing a process does not empty the operating system's page cache. A vault that survives every kill in this suite has proved that the *ordering* holds — that no index generation names bytes the code had not yet synced — and has not proved that the platform's `fsync` reaches the platter. Whole-machine power loss is the only test for that, and there is no rig for it. The suite proves what it proves.

---

## P4.3 — Reclaiming space

*Plan P4.3 · Spec §4.5, §5.1 · Design §7, §8.4 · FR-23, FR-24, FR-25*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P4.3.a | The operation Spec §5.1 names, returning what it recovered rather than leaving the caller to subtract two statistics | Spec §5.1, FR-8 | T4.9 |
| P4.3.b | Packs selected by garbage ratio, highest first; a pack holding no garbage is not rewritten | Spec §4.5, FR-25 | T4.12 |
| P4.3.c | Every pack holding any garbage is rewritten, so the figure FR-8 put in front of the user is the figure the operation delivers | FR-8, Design §8.4 | T4.9 |
| P4.3.d | One pack per generation step: live extents copied into a new pack, made durable, one index commit, then the old pack removed | Spec §4.5, FR-24, HC-4 | T4.11, T4.19 |
| P4.3.e | Stored bytes copied as they are — no decryption, no re-encryption, no new nonce, no new entry identifier | Spec §3.3, §4.5 | T4.11 |
| P4.3.f | Working space bounded by roughly one pack whatever the vault's size | FR-25 | T4.10, T4.25 |
| P4.3.g | Progress and cancellation, checked between packs and within one. Cancelling keeps every pack already reclaimed and leaves the one in flight as an orphan for P4.4 | A-3, FR-14, FR-24 | T4.13 |
| P4.3.h | A pack that cannot be read in full refused rather than rewritten, naming the entries it costs | S-4, HC-3 | T4.14 |
| P4.3.i | Statistics updated in the same commit that moves the extents, never recomputed by scanning content | FR-8, FR-22 | T4.9 |
| P4.3.j | `veil reclaim-space` on the command line, stating the figures before and after, with no flag that schedules, times, or conditions it | A-4, FR-23, Design §7, §8.4 | T4.15, T4.16 |
| P4.3.k | `delete`'s wording corrected: it currently tells the user this version cannot reclaim space, which stops being true in this phase | FR-21, FR-29, Design §7 | T4.16 |

**Why there is no minimum ratio.** The obvious refinement is to skip a pack whose garbage is a rounding error — rewriting a gigabyte to recover four kilobytes is work for nothing. It is refused anyway, because Design §8.4 puts the figure in the control the user presses: *Reclaim 18.2 GB*. An operation that then reclaims 17.9 GB because some packs fell under a threshold has made the number in the button a lie, and this product does not have numbers that are approximately true. The protection against pointless work is FR-8 itself — a user looking at four kilobytes of reclaimable space does not press the button — and FR-23 guarantees nobody but the user ever presses it.

**Why the bytes are copied rather than re-encrypted.** Re-encrypting would need every entry's key unwrapped and every nonce reissued, and the entry identifier is bound into both the wrapping nonce and the content's associated data. Copying ciphertext verbatim keeps identifiers, keys and nonces exactly as they were, so reclaiming space cannot introduce a cryptographic fault; the only thing that changes is which pack an extent lives in. It is also the fast path, but that is not the reason.

**Why P4.3.h refuses rather than continues.** Reclaiming space over a pack with a damaged region would copy the damage into the new pack, remove the original, and report success. The damage survives, the evidence of where it came from does not, and the user has been told the vault is now tidier. Damage is `check`'s subject (FR-33) and it must not be laundered by an unrelated operation.

---

## P4.4 — Nothing at open

*Plan P4.4 · Spec §4.5 · HC-4, FR-8, FR-22, FR-26, FR-27*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P4.4.a | **Nothing at open.** No write — not the index, not a pack — and no walk of the packs directory. Opening a vault is the header and one index slot | HC-4, S-2, FR-22, FR-27 | T4.17, T4.18, T4.28 |
| P4.4.b | Bytes on disk that no committed operation put there found by reclaiming, which measures the packs before selecting anything, and swept with everything else | FR-8, FR-23, HC-4 | T4.17, T4.19, T4.27 |
| P4.4.c | The same bytes found by reporting the figures, so a user who asks what the vault occupies is told the truth without being made to reclaim to learn it | FR-8, Design §3.2 | T4.17, T4.27 |
| P4.4.d | The maintained figures left understating rather than corrected behind the user's back, and the statistics trued up only inside the generation step that reclaiming commits | FR-22, S-2 | T4.6, T4.27 |
| P4.4.e | A vault whose storage will not take a write opened read-only, and said so at open rather than left to be discovered by a failing command | FR-26, Spec §4.5, §4.8 | T4.20 |
| P4.4.f | What the user deleted left alone until they ask for the space back, which is the same rule and needs no discrimination between the two kinds | FR-21, FR-23, FR-29 | T4.21, T4.26 |

**Why nothing happens at open, and why this is not what the phase proposed.** This phase found that discarding unreferenced bytes acts on a *guess* — an index behind its packs is indistinguishable from a killed ingest, and §1 names a vault in a sync folder as a motivation for the product — and proposed reporting instead of discarding. That kept the mechanism and changed its verb. Two further objections remove it entirely: a write at open advances the generation FR-27 detects change with, and a walk of the packs directory puts vault size into the cost of every open, against S-2. Both are objections to *where the work sits*, not to what it does. Moved into reclaiming and into reporting the figures — both user-initiated, both already measuring the packs — the same work is correct, and opening a vault is a read of two files.

**Why the figures are allowed to understate.** After an interruption the maintained totals count less than the filesystem holds, and nothing corrects them until the user asks. The alternative costs one `stat` per pack at every open, and pack count follows vault size (S-2). The error is in the safe direction: the product promises less space than reclaiming returns.

**Why P4.4.e is in this list at all.** FR-26 requires a vault that cannot be written to open read-only *and say so*. Phase 3 shipped the first half: a read-only vault opens, and the first write fails with its own exit code. Nothing told the user before they tried. That is the difference between a product that explains itself and one that has to be experimented with.

---

## P4.5 — A pack that is gone

*Plan P4.5 · Spec §4.5 · S-4*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P4.5.a | A referenced pack missing from disk does not prevent the vault from opening | Spec §4.5, S-4 | T4.22 |
| P4.5.b | The entries with extents in it enumerable at open, without reading content | S-4, S-2 | T4.22 |
| P4.5.c | Every entry outside it still listed, still extractable, still verified | S-4 | T4.23 |
| P4.5.d | Never confused with space to reclaim: a missing pack is referenced by definition, so reclaiming never counts it and the figures are never adjusted to match it | FR-8, S-4 | T4.24 |
| P4.5.e | Reported as damage in the words Design §7 fixes, pointing at `check` for the full list | Design §4.2, §7, FR-33 | T4.22 |

**Why refusing to open would be the defect.** A vault that will not open because one of four hundred packs is missing has converted the loss of one pack into the loss of everything else, which is precisely the failure S-4 exists to reject. The right answer is the one §4.5 already fixes: open, name the casualties, and keep serving the rest.

**Why the statistics are not silently corrected here.** Reclaiming trues the totals up when it finds *more* on disk than the index accounts for. A missing pack is the opposite — less on disk, and it is damage rather than space — so the figures are left exactly as they are and the shortfall is reported. Damage that adjusts the numbers to match itself is damage that has covered its tracks.

---

## P4.6 — The crash suite

*Plan P4.6 · Spec §9 · HC-4*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P4.6.a | Green across add, replace, delete and reclaim space — the four operations Spec §9 names | Spec §9, HC-4 | T4.2–T4.5 |
| P4.6.b | Every case asserting all four invariants of P4.2.c, not merely that the vault opened | HC-4 | T4.6 |
| P4.6.c | A failure naming the operation and where the kill landed, so a flake and a defect are distinguishable | Spec §9 | T4.7 |

---

## Exit

The Plan's exit conditions for this phase, and where each is met:

- **No interruption at any fsync boundary yields an unopenable vault or loses an entry that existed beforehand** — T4.2 to T4.8, with P4.2's stated limit: the kill is a process kill, not a power cut.
- **Compaction needs working space bounded by roughly one pack** — T4.10 at test scale and T4.25 at a scale where the difference is unambiguous.
- **An interrupted compaction leaves nothing unreachable, and the space it left is recovered by the next reclaim** — **met** (T4.19, T4.27). The condition read differently when this phase was written: FR-32 required cleanup at open, this phase declined and reported instead, and the owner withdrew the requirement altogether. Nothing happens at open now, which T4.17, T4.18 and T4.28 assert directly.
- **A vault on read-only media opens** — T4.20, which also covers the half of FR-26 that says it must say so at open rather than at the first failed write.

---

## Notes for Upstream

Recorded per G-24. **All ten were absorbed by the owner into Requirements v2.0 and Specification v2.0**, and the pins in the header are the versions that contain them. Each was proposed with the reading this phase implemented, so that the work was not blocked and the decision stayed the owner's; the owner ruled for every reading, so no code changed on absorption. Where each landed:

| Note | Landed in |
|---|---|
| 1 — directory-level durability | Spec §4.7 |
| 2 — nowhere to report what open found | **dissolved** — nothing is found at open; Spec §5.1, `recount_statistics()` reports on request |
| 3 — no operation enumerates a missing pack's entries | Spec §5.1, `missing_packs()` / `unreadable_entries()` |
| 4 — reclaiming while a read is in flight | Spec §11.1, resolved: it may not |
| 5 — fsync ordering unverified | Spec §11.1, resolved: ordering proved, platform's honouring of it not |
| 6 — pack identifiers rise by argument | superseded by note 9 |
| 7 — FR-32 says discard, this phase reports | **Requirements v2.0, FR-32 withdrawn** — neither verb; nothing happens at open. Spec §4.5; FR-8, FR-22, FR-26 and Design §3.2, §8.4 carry what it covered |
| 8 — what the crash tests kill | Spec §9 |
| 9 — pack identifiers can be reused, in one case | **Spec §4.3, `next_pack_id`** — fixed rather than recorded; Plan P5.7 builds it |
| 10 — nothing writes the index at open | Requirements FR-22 and the withdrawn FR-32, Spec §4.5 — generalised from "no write" to "nothing at all" |

Two are rulings rather than clarifications, and in both the owner went past what this phase proposed.

**Note 7** was the largest departure in the phase. This list proposed keeping reconciliation at open and changing its verb from discard to report. The owner withdrew FR-32 instead: the objection was to doing anything at open, not to which thing. The work moved to reclaiming and to reporting the figures, both of which the user asks for. That this list proposed the smaller change is worth leaving on the record — it is the same shape of error as the one that created FR-32, and it was made by the people who had just diagnosed it.

**Note 9** proposed recording the pack-identifier defect and not fixing it, on the ground that its consequence is a detected error rather than silent corruption. The owner overruled that and required the counter: reporting damage where there is none is FR-2's failure in a different place, and the format is still private enough for the fix to cost a line.

1. **Spec §4.7 fixes the ordering between pack and index and says nothing about the directory.** `fsync` on a file makes its contents durable, not its name; the directory entry needs its own sync. Three write paths are affected — a newly created pack, the header's rename, and an index slot's first creation — and none of them does it today. *Proposed: §4.7 gains a sentence putting the containing directory inside the same ordering obligation as the file.* Resolver: owner, at the next Specification bump.

2. **FR-32 requires open to report the space it recovered, and §5.1's API has nowhere to put it.** *(Dissolved rather than answered: FR-32 is withdrawn and open reports nothing.)* `Vault::open` returns a `Vault`. *Proposed: an accessor on the open vault carrying what reconciliation found — clean, residue of a stated size, or not examined because the vault is read-only — leaving the constructor's signature as §5.1 fixes it.* Resolver: owner.

3. **§4.5 requires the entries of a missing pack to be enumerated at open, and §5.1 has no operation that does it.** *Proposed: an accessor alongside the one in item 2, computed from extents and one existence check per referenced pack, so it costs no content read (S-2).* Resolver: owner.

4. **§11.1: whether compaction may proceed while a read is in flight.** *Proposed resolution — it may not, and nothing is added to permit it.* §5.1 already fixes the operation as taking an exclusive borrow of the vault, so no read through the same vault can be in flight, and a second process is refused by the advisory lock long before this matters. What Design §8.4 promises — that the vault stays usable while space is being reclaimed — is browsing, and browsing is served from the resident index without touching a pack (FR-6). How the graphical application holds the vault while a worker reclaims is P6.1's problem and is an architecture question, not a format one. Resolver: owner, to close the §11.1 item.

5. **§11.1: the fsync ordering of §4.7 is unverified.** *Proposed resolution — verified to the extent a process kill can verify it (P4.2), and the residue stated rather than closed:* the ordering is proved, the platform's honouring of `fsync` is not, and there is no rig for the latter. Resolver: owner, to close or to re-word the §11.1 item.

6. **Pack identifiers are never reused, and this holds by argument rather than by a counter.** A new pack always takes one above the highest that exists, and reclaiming space creates the new pack before removing the old, so the highest only ever rises. Entry identifiers needed a stored counter for the same property (Phase 2, note 5) because deleting the highest entry *does* lower the maximum. The asymmetry is real and worth recording, because the obvious tidy-up — "allocate from the same counter for both" — would be adding a format field for a property already held. Resolver: none needed; recorded so it is not undone by accident. **Qualified by item 9**, which is the one case where it does not hold.

7. **FR-32 says to discard the residue at open, and this phase reports it instead.** *Proposed as implemented, and this is the largest departure in the phase.* Whether unreferenced bytes are residue is a **guess**: an index that is behind its packs looks exactly like a killed ingest. §1 names a vault in a sync folder as a motivation for the product, and a daemon can deliver an older index before the packs a newer one describes — discard at that moment and content the newer index still points at is gone, with no interruption anywhere in the story. HC-4 forbids losing data to a single event; here there was not even one. FR-32's own words name the target as "the residue an interrupted ingest or compaction leaves behind **under HC-4**", so where the identification is uncertain HC-4 governs. What happens instead: the residue is found, reported, and counted into the space the user can reclaim, and `reclaim-space` takes it — which is the deliberate act FR-23 already requires for recovering space. Nothing accumulates unseen and nothing is destroyed on a guess. If the owner prefers FR-32's letter, the change is one branch and T4.28 is the case that will fail. Resolver: owner.

8. **Spec §9 names the crash tests but not what they kill.** This phase kills the shipped binary for three of the four operations and a test-only subject for the fourth, because the pack cap §4.5 deliberately made settable is settable only through the API. *Proposed: §9 gains a sentence saying so, since "a real kill, not a simulated one" is the property that matters and both satisfy it.* Resolver: owner.

---

## Open Questions

- **Whether `reclaim-space` should report per-pack progress or whole-operation progress.** Per pack is honest about the unit of work that survives an interruption (P4.3.d); whole-operation is what the person watching wants to know. The implementation reports bytes recovered against bytes reclaimable, which is the second, and states the first in its result. Recorded because it is a Design §3.4 judgement made here rather than upstream. Resolver: owner, at Phase 7 when the graphical presentation of §8.4 is built.

9. **Pack identifiers can be reused, in one case.** Reclaiming space removes a pack whose contents are entirely dead, and if that pack held the highest identifier the next allocation takes the number back. A stale index slot could then name a pack whose bytes are now something else — which fails authentication and is reported as damage rather than returning wrong content (HC-3), so the consequence is bounded, but it is not nothing. The general claim in note 6 — that identifiers only ever rise — holds for every path *except* removing the highest pack. Closing it needs a stored counter, as entry identifiers have. *Proposed: recorded, not fixed, since the failure mode is a detected error rather than silent corruption.* Resolver: owner.

10. **Nothing writes the index at open, and FR-32's report needed that.** An index write at open advances the generation, and the generation is FR-27's whole mechanism — a vault opened from a stale copy would come away holding a number higher than the newer index a daemon then delivers, and every later write would pass a check meant to refuse it. Found by two Phase 2 cases (T2.5, T2.41) failing the moment reconciliation committed. Worth recording upstream because the trap is invisible from §4.5, which describes reconciliation as a write without noting what a write at open costs. *Proposed: §4.5 gains a sentence.* Resolver: owner.
