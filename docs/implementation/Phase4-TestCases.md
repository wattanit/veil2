# Veil2 — Phase 4 Test Cases: Durability and Compaction

**Version:** 2.0
**Status:** approved
**Date:** 2026-08-09
**Owner:** wattanit
**Foundation and plan versions these cases are built against (G-14):**
- Requirements Document **v2.0** — upstream
- Design Guideline **v2.0** — upstream
- Technical Specification **v2.0** — upstream
- Implementation Plan **v2.0** — upstream
- [Phase4-ToDo.md](Phase4-ToDo.md) **v2.0** — companion; each case names the item it covers

*Changes since v1.0 (**major**):* approved, re-pinned, and the reconciliation cases rewritten. FR-32 is withdrawn (Requirements v2.0) and nothing happens when a vault opens, so T4.17, T4.19, T4.20, T4.21, T4.26 and T4.27 now assert that the space an interrupted operation left is found by *asking* — reclaiming, or reporting the figures — and that opening finds and changes nothing. T4.18 is unchanged in intent and stronger in fact: an open writes nothing and now measures nothing either.

> **v1.1 of this document, published earlier the same day, is wrong.** It described T4.17 and T4.28 as verifying a reversed FR-32. There is no FR-32.

This document owns the **enumerated checks that close Phase 4**. Every case cites the requirement it verifies (G-10).

---

## Conventions

**Case identifiers** are `T<phase>.<n>` — section-numbering references, not foundation identifiers (G-19).

**The crash cases kill a real process.** No case in this file simulates an interruption, and nothing in `veil-core` exists to let one. A case that cannot be reached without a seam in shipped code is not written; it is recorded as unreachable and said so, which this file does twice.

**What "the vault survived" means.** Every crash case asserts the same four things, and a case that asserts fewer is not asserting HC-4:

1. the vault opens;
2. every file that existed before the killed operation is still listed;
3. each of those extracts byte-identically to what was stored;
4. the arithmetic holds: file count and stored bytes match a measurement exactly, and the on-disk total is never *higher* than what is there — after a kill it is lower by the bytes the killed operation left, and those are counted as space to reclaim. Nothing corrects the figures at open, so this asserts the gap is the right shape rather than that it is absent.

**Where these run.** The development machine, macOS. Windows and Linux are unconfirmed (Spec §8.1), and it matters here more than anywhere: signal delivery, `fsync` semantics, and whether a directory sync is required at all are platform behaviour. What passes here is evidence for this platform and a reasonable expectation for the others.

**How to run them.**

```bash
cargo test --release -p veil-cli                              # the crash suite, and Phase 3's
cargo test --workspace                                        # everything, debug
cargo test --release -p veil-cli --test crashes -- --ignored   # the sweep (T4.7)
cargo test -p veil-core --test reclaim -- --ignored            # the scale case (T4.25)
```

The crash cases spawn real processes that derive real keys, so they run in release for the same reason the Phase 3 suite does — and the whole package rather than one target, because selecting a single test target skips the example the reclaim cases drive.

The scale case runs in **debug**, and not by preference: `cargo test --release -p veil-core` does not build, because the core's suite needs the cheap KDF parameters that are deliberately compiled out of anything but a debug build. It is slower and it still finishes.

---

## Write ordering

### T4.1 — Every write path is a known write path
*Covers P4.1.a, P4.1.b · Verifies FR-12, HC-4*

Audit `veil-core`'s source for every point at which a file is created, renamed over, or removed, and compare against the enumerated list of write paths.

**Verdict:** the set matches exactly. A new one that nobody reviewed fails the case, and the fix is to review it and add it, not to widen the check. This is a tripwire, not a proof — what proves the ordering is T4.2 to T4.8. It exists because the ordering obligation is the kind that is met once and then quietly broken by a later change that had no idea it was participating.

---

## Crashes

### T4.2 — A kill during an add loses nothing that was already there
*Covers P4.2.a, P4.2.b, P4.2.c, P4.2.f, P4.1.b · Verifies HC-4, FR-12*

Build a vault holding several files. Start adding a file large enough that the add is genuinely in flight, wait until the vault's own bytes start appearing on disk, then kill the process with an uncatchable signal.

The trigger is the vault growing, not anything the process was built to tell a test — there is nothing to tell it with, and adding something would be the seam Spec §11.1 rejected. Progress output would have been the other candidate and is unusable: off a terminal it is one line every two seconds (P3.5.c), so waiting for it would need a multi-gigabyte fixture.

**Verdict:** the four invariants. The interrupted file is either wholly present or wholly absent — never listed with content that does not authenticate — and the vault is openable without any repair step being asked of the user. The signal is a kill and not an interrupt: an interrupt is cancellation, which is a *stronger* promise, and T3.18 covers it.

### T4.3 — A kill during a replace leaves exactly one intact version
*Covers P4.2.a, P4.2.c · Verifies HC-4, FR-13*

Store a file, then replace it with different content and kill the process part-way.

**Verdict:** the path holds either the old content or the new content, in full, and it extracts. Never zero versions, never a truncated one. FR-13 says the new content is durable before the old becomes unreachable, and this is the only case that can tell whether that ordering was actually implemented or merely intended.

### T4.4 — A kill during a delete leaves the file present or gone, never half
*Covers P4.2.a, P4.2.c · Verifies HC-4, FR-21*

Delete a file from a vault and kill the process during the operation.

**Verdict:** the four invariants, and the file is either still listed and extractable or absent with its bytes accounted as reclaimable. A delete is one index generation, so this case is mostly asserting that it really is one — an implementation that removed the entry and updated the statistics in two commits would show up here and nowhere else.

### T4.5 — A kill during reclaiming space loses no live file
*Covers P4.2.a, P4.2.d, P4.6.a · Verifies HC-4, FR-24*

Build a multi-pack vault with a small pack cap through the subject binary, delete enough to make several packs worth reclaiming, start reclaiming, and kill part-way.

**Verdict:** the four invariants. Every live file extracts byte-identically whether its extents had been moved yet or not, and the vault opens with no manual step. FR-24 requires the vault to be openable at *every* point during the operation, and a kill at an arbitrary point is the only way to sample that.

**Why this one uses a subject that is not the shipped binary.** Reclaiming space is a multi-pack behaviour and the pack cap is 1 GiB. Spec §4.5 made the cap an API parameter exactly so this test would not need gigabytes, and the command line does not expose it — a flag existing only to make a test cheap is the seam this project refuses. So the subject is a small binary that links `veil-core` and takes the cap as an argument. It is killed for real; nothing is simulated. Recorded in the To-Do as *Notes for Upstream*, item 8.

### T4.6 — After any kill, the statistics are true again
*Covers P4.2.c, P4.6.b, P4.4.c · Verifies FR-8, FR-22, HC-4*

After each of T4.2 to T4.5, open the vault and compare every reported figure against a full recount.

**Verdict:** entry count and logical bytes agree exactly; physical and reclaimable bytes are lower than the measurement by the same amount, which is the bytes the killed operation left and never committed. Then reclaim, and all four agree exactly — because reclaiming measures first and commits what it found.

An incremental counter is broken by exactly the event this suite creates, so a suite that killed processes and never checked the arithmetic would be testing the easy half. What changed at v2.0 is what "true again" means: opening does not make the figures true, because opening measures nothing (FR-22). Reclaiming does, and it is the user's act.

### T4.7 — Repeated kills at unpredictable points
*Covers P4.2.e, P4.6.c · Verifies HC-4* — `#[ignore]`, run on request

Run each operation many times, killing at a different point each run, seeded so a failure reproduces exactly.

**Verdict:** the four invariants every time. A failure names the operation and the point at which the kill landed. This is the case that finds the boundary nobody thought of; it is `#[ignore]` because it costs minutes, and it is seeded because an unreproducible crash-test failure is worth almost nothing.

### T4.8 — Both index slots are never unreadable at once
*Covers P4.2.c · Verifies HC-4, Spec §4.4*

After every kill in the suite, read both index slots directly.

**Verdict:** at least one authenticates. The double-buffered design exists so that a crash mid-write damages only the expendable slot; this asserts it against real interruptions rather than against a test that overwrites a slot on purpose.

---

## Reclaiming space

### T4.9 — What was promised is what is recovered
*Covers P4.3.a, P4.3.c, P4.3.i · Verifies FR-8, FR-25, Design §8.4*

Read the reclaimable figure, reclaim space, then read the figures again.

**Verdict:** the space recovered equals the figure that was showing beforehand, the reclaimable figure is zero afterwards, and the physical figure fell by the amount recovered. Design §8.4 puts that number in the control the user presses, so a run that recovers less than it said has made the interface untrue.

### T4.10 — Working space stays bounded by about one pack
*Covers P4.3.f · Verifies FR-25*

Build a multi-pack vault at a small cap with garbage spread across every pack, and sample the total bytes on disk throughout the operation.

**Verdict:** the peak never exceeds the starting total by more than about one pack. FR-25 is the requirement that makes reclaiming space possible at all at the sizes in §1; an implementation that copies everything and then swaps passes every other case in this file and fails this one.

### T4.11 — Live content survives byte-identically, and so do its identifiers
*Covers P4.3.d, P4.3.e · Verifies FR-25, Spec §3.3, §4.5*

Reclaim space over a vault with several files, then extract every one and compare against what was stored.

**Verdict:** every file is byte-identical, every entry identifier is unchanged, every content hash is unchanged, and only the extents differ. Reissuing an identifier would break the binding it has into the wrapping nonce and the content's associated data, and this case is where that would show.

### T4.12 — A pack with nothing to recover is not rewritten
*Covers P4.3.b · Verifies FR-25*

Reclaim space over a vault where one pack holds no garbage at all.

**Verdict:** that pack is the same file afterwards — same identifier, same bytes — and the operation reports it as untouched. Rewriting a pack to recover nothing is pure cost, and at the sizes in §1 it is minutes of it.

### T4.13 — Cancelling keeps what was already reclaimed
*Covers P4.3.g · Verifies FR-14, FR-24*

Start reclaiming space over several packs and cancel it part-way.

**Verdict:** the cancelled exit code, the packs already reclaimed stay reclaimed, the vault opens, and the partly-written pack is gone by the time the next open finishes. Each pack is its own transaction, so a cancellation costs at most the pack in flight — which is FR-24's "at most the current unit of work" made observable.

### T4.14 — A pack that is not all there is refused, not compacted away
*Covers P4.3.h · Verifies S-4, HC-3*

Truncate a pack so that an extent the index holds claims more than the file contains, then try to reclaim space.

**Verdict:** the operation refuses that pack, names the entries with extents in it, and never rewrites it. Copying a short extent forward would produce an entry whose recorded length no longer matches its stored bytes, and would delete the original that proved what happened.

**What this does *not* refuse, and why.** A pack that is complete but whose bytes were *altered* is reclaimed like any other. Telling authentic bytes from tampered ones means decrypting them, and reclaiming space deliberately does not decrypt (P4.3.e) — so the choice is between doing a full verification pass inside an operation whose job is not verification, or copying the damage faithfully. Copying loses nothing: the same bytes stay damaged in the new pack, `check` still names the same entries, and FR-33 keeps damage as the subject of the operation built for it. Asserted here rather than left implicit, because "reclaiming refuses damage" is the kind of claim that gets remembered as broader than it is.

### T4.15 — Reclaiming has no schedule and no condition
*Covers P4.3.j · Verifies FR-23, Spec §5.2*

Read the full help output of `reclaim-space` and of every other command.

**Verdict:** no flag schedules, times, daemonises, or triggers on a threshold — the same verdict as T3.3, re-asserted now that the operation FR-23 is actually about exists. This is the case FR-23 was written for; until this phase there was nothing to attach it to.

### T4.16 — The command says what it did and what it did not
*Covers P4.3.j, P4.3.k · Verifies FR-21, FR-22, FR-29, Design §7, §8.4*

Reclaim space and read the output. Then delete a file and read that output.

**Verdict:** reclaiming states the figures in the words Design §7 fixes and never says compact, vacuum, or garbage-collect. Deleting still states that the bytes remain until space is reclaimed, and no longer says this version cannot reclaim it — a true sentence in Phase 3 that becomes a false one here, which is the kind of message that survives for years because nothing tests prose.

---

## What happens when a vault is opened

### T4.17 — Residue is found at open and reported, not destroyed
*Covers P4.4.a, P4.4.b, P4.4.c · Verifies FR-8, FR-22, HC-4*

Leave a pack file that no extent references beside an otherwise intact vault, then open it.

**Verdict:** opening changes nothing and reports nothing — the figures the vault carries are the same before and after, and the bytes are still on disk. Asking is what finds them: a measurement counts them as space that can be reclaimed, and `reclaim-space` takes them. FR-32 once required them to be discarded here; it is withdrawn, and T4.28 is the case that decided it. Space that reappears without explanation is indistinguishable, to the person watching, from space that was never accounted for properly — so it is reported where a user asked, and nowhere else.

### T4.18 — An open never writes
*Covers P4.4.d · Verifies HC-4, S-2, FR-27*

Open an intact vault twice, recording the generation and both index slots. Then plant residue and open again.

**Verdict:** identical both times. The generation does not advance, no slot is rewritten, no pack is touched — including on the open that finds residue, which is the one that tempts a write. An open that writes is an open that can fail; worse, it costs FR-27 its detector, because a vault opened from a stale copy would come away holding a generation higher than the newer index a sync daemon then delivers.

### T4.19 — An interrupted reclaim leaves nothing unreachable
*Covers P4.4.b, P4.1.c, P4.3.d · Verifies FR-8, HC-4, FR-24*

Leave behind what a kill on either side of the reclaim commit produces — a new pack the index had not adopted, or an old one it had already let go of — then open the vault and reclaim.

**Verdict:** every live file reads, the leftover is reported as reclaimable, and reclaiming takes it. This is the case that proves the ordering of P4.1.c: the operation is recoverable from both sides of its commit, or it is not.

### T4.20 — A read-only vault opens, says so, and is not written to
*Covers P4.4.e · Verifies FR-26, Spec §4.5, §4.8*

Make a vault directory read-only, leave an orphaned pack in it, and open it. List it and check it. Then try to add.

**Verdict:** it opens; the orphaned pack is still there; listing and checking succeed; the add exits with the read-only code; and the fact that the vault opened read-only is stated at the time it opens, not left to be discovered by the failing add. Refusing to open would turn an interrupted reclaim on a drive that later became read-only into permanent data loss, which HC-4 forbids.

### T4.21 — Garbage inside a live pack is left alone
*Covers P4.4.f · Verifies FR-23, FR-21*

Delete one file from a pack that holds several, then open the vault.

**Verdict:** the pack is untouched, its size is unchanged, and the reclaimable figure still counts the deleted file's bytes. Recovering those bytes means rewriting the pack around them, which is reclaiming space and the user's decision alone (FR-23). T4.26 makes the same assertion for the shapes where no rewriting would be needed at all, which is the harder half.

### T4.26 — Deleted bytes are not residue and are never taken at open
*Covers P4.4.f · Verifies FR-21, FR-23, FR-29*

Delete every file with extents in one pack, close the vault and open it again. Then do the same to a file at the *end* of a pack that still holds others.

**Verdict:** nothing is removed, nothing is truncated, and the reclaimable figure is exactly what it was. Both shapes are trivially discardable — a whole dead pack, and a dead tail with nothing live above it — and neither may be discarded, because those are bytes the product told the user would stay until they asked for the space back. The two shapes are chosen deliberately: they are what a rule phrased as "discard what nothing references" would take.

### T4.27 — The residue of an interrupted ingest is found, tail and all
*Covers P4.4.b, P4.4.c · Verifies FR-8, HC-4*

Append bytes to a live pack that no commit ever learned of — what a kill part-way through an add leaves behind — then open the vault.

**Verdict:** the exact byte count is reported, it is counted into what can be reclaimed, and `reclaim-space` takes it. The mirror of T4.26, and the pair is where the discrimination is asserted: the statistics count what was committed, the filesystem counts what is present, and the difference is the residue. It matters that this shape is a *tail on a live pack* rather than a whole pack — that is where an interrupted add leaves it, and a rule about whole packs misses it entirely.

### T4.28 — A vault whose index is behind its packs loses nothing
*Covers P4.4.a · Verifies HC-4, FR-27*

Deliver a vault the way a sync daemon would: the packs of a newer state, but an older index. Open it. Then let the newer index land, and open it again.

**Verdict:** the file the newer index names is still there and still reads. This is the case that decides why residue is reported rather than discarded. At the first open those bytes are indistinguishable from the residue of a killed ingest, and discarding them would destroy content that no interruption ever touched — a vault in a sync folder is something §1 names as a motivation for the product, not an exotic case. If the owner rules for FR-32's letter, this is the case that will fail, and it should be made to fail deliberately rather than by accident.

---

## A pack that is gone

### T4.22 — A missing pack opens the vault and names its casualties
*Covers P4.5.a, P4.5.b, P4.5.e · Verifies S-4, Spec §4.5*

Remove one pack file from a multi-pack vault and open it.

**Verdict:** the vault opens; the entries with extents in that pack are enumerated at open without any content being read; and the command line says so plainly, in Design §7's words, pointing at `check` for the full account. Refusing to open would convert the loss of one pack into the loss of the whole vault — the failure S-4 exists to reject.

### T4.23 — Everything outside the missing pack still works
*Covers P4.5.c · Verifies S-4*

In the same vault, list, save a copy of a file stored elsewhere, and check.

**Verdict:** listing is complete, the copy is byte-identical, and `check` names exactly the entries in the missing pack and no others. S-4 is not the claim that damage is detected; it is the claim that damage is *bounded and attributable*, and only the second half is worth anything to someone deciding whether to reach for a backup.

### T4.24 — A missing pack is never treated as garbage
*Covers P4.5.d · Verifies FR-32, S-4*

Open the vault of T4.22 and ask it what is missing.

**Verdict:** nothing was removed, nothing was recovered, no entry was dropped from the index, and the statistics were not quietly adjusted to match the smaller vault. A missing pack is referenced by definition, so it is damage and not residue, and an implementation that confuses the two deletes the record of what the user lost.

---

## Scale

### T4.25 — Bounded working space at a size where it is unambiguous
*Covers P4.3.f · Verifies FR-25* — `#[ignore]`, run on request

Build a vault large enough that one pack is a small fraction of it, make most of it garbage, and reclaim space while sampling the total on disk.

**Verdict:** the peak stays within about one pack of the starting total, and the vault is intact afterwards. T4.10 asserts the same property at test scale where a bug could hide inside the noise; this one is the Plan's exit condition, and it is `#[ignore]` because it costs minutes and disk.

---

## Not covered, and why

**Power loss.** Every crash case kills a process. That proves the *ordering* — no index generation names bytes that had not been synced — and does not prove that the platform's `fsync` reached the medium, because the page cache survives a process and only a power cut empties it. There is no rig for that here. Spec §11.1's open item is answered to the depth a process kill can answer it and no further, and the To-Do says so rather than letting the suite's green imply more than it tested.

**A reclaim interrupted between the index commit and the pack removal, deterministically.** T4.19 reaches both sides of that boundary by killing at unpredictable points, and T4.7 samples it repeatedly. Landing on it *on purpose* would need the code to tell a test where it is, which is the seam Spec §11.1 already rejected. The window is small and the assertion is statistical; that is stated rather than dressed up as coverage.
