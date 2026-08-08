# Veil2 — Phase 2 Test Cases: Vault Operations

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-08
**Owner:** wattanit
**Foundation and plan versions these cases are built against (G-14):**
- Requirements Document **v1.1** — upstream
- Design Guideline **v1.1** — upstream
- Technical Specification **v1.1** — upstream
- Implementation Plan **v1.3** — upstream
- [Phase2-ToDo.md](Phase2-ToDo.md) **v1.0** — companion; each case names the item it covers

This document owns the **enumerated checks that close Phase 2**. Every case cites the requirement it verifies (G-10). It defers what must be true to the Requirements and how it is built to the Specification; a case that cannot cite a foundation identifier does not belong here.

---

## Conventions

**Case identifiers** are `T<phase>.<n>` — section-numbering references, not foundation identifiers (G-19).

**Every case drives the public API.** Phase 1's suite mutates bytes on disk because the attacker's position is the file. Phase 2's subject is the API, so its cases go through the same surface a frontend would use (A-1, A-4). Where a case must observe something the API does not return — a file that was opened, a byte that was written — it observes the filesystem, never a crate-private path.

**No case requires a terminal, a prompt, or a subprocess.** That is the phase's exit condition, so a case that needed one would be evidence against the thing it was meant to check.

**Every failure case asserts which failure.** "Returns an error" is satisfied by a build that fails on everything.

**Where these run.** In the CI matrix on macOS, Windows, and Linux as peers (HC-8). Advisory-lock behaviour differs between platforms and T2.1 is therefore a genuine three-platform case, not a Linux case run three times.

---

## Lifecycle and locking

### T2.1 — A second opener is told the vault is in use
*Covers P2.1.a, P2.1.b · Verifies FR-26, Spec §2, §6*

Open a vault. With it still open, open the same directory again from the same process, then from a second process.
**Verdict:** `VaultInUse` in both, distinct from every other error. Not a corruption error and not an I/O error — a user whose vault is open in another window must be told that, not sent to look for damage.

### T2.2 — The lock does not outlive the vault, including on failure
*Covers P2.1.c · Verifies FR-26, HC-4*

Open a vault and close it; reopen. Then open a vault, force an operation to fail mid-way, drop the vault, and reopen. Then open a vault inside a scope that panics, and reopen after unwinding.
**Verdict:** every reopen succeeds. A leaked lock reports a user's own vault as in use, and the remedy is a file they were never told about.

### T2.3 — Locking a vault destroys its keys
*Covers P2.1.d · Verifies FR-3, HC-2, Spec §5.1*

Call `lock` on an open vault.
**Verdict:** the value is consumed, so a locked vault is not reachable as a value with a flag set, and the vault is openable again — the lock went with it. Zeroisation itself is asserted against the key types by T0.5 and T0.6, which is where the memory is; asserting it from outside would mean reading freed memory.

### T2.4 — Open reads the index and nothing else
*Covers P2.1a.a, P2.1a.c, P2.1.f, P2.13.f · Verifies FR-6, FR-22, S-2, A-7, FR-33*

Build a vault spanning several packs, close it, delete every pack file, and open it again.
**Verdict:** it opens, enumerates every entry, and reports statistics identical to before — nothing that read a pack could survive it, which is S-2 in its strongest observable form and simultaneously the FR-33 assertion, since a verification at open could not pass. Separately, two vaults are open at once in one process (A-7).

**Removing the packs rather than timing the open is deliberate.** A timing assertion on shared CI hardware is a flake generator, and what S-2 states is not that open is fast but that vault size is not an input to it.

### T2.5 — A vault changed on disk since open is not written over
*Covers P2.1.e · Verifies FR-27, Spec §4.3, §4.4*

Open a vault whose index slots are then replaced underneath it with a newer generation, as a sync daemon replicating another machine's write would. Attempt a write.
**Verdict:** `ChangedOnDisk` from `add`, `replace`, and `delete` alike — a check that holds for one write path and quietly not for the rest is not a check — and the outside entry is still present afterwards. Silently winning would discard a write the user made, which is the sync-folder failure FR-27 exists for.

**The external writer is a file copy, because that is what it is in life.** It is also why the advisory lock does not see it, and why §2's honesty clause names the generation counter as the actual protection.

### T2.41 — A changed vault reloads without the password
*Covers P2.1.g · Verifies FR-27*

After T2.5's refusal, reload and retry the same write.
**Verdict:** the reload adopts the outside change — its entry is present and its content readable — and the retried write then succeeds on top of it rather than over it.

**Detection is only half of FR-27**, which requires the product to *offer to reload*. Requiring the password again to get past the refusal would make the safe answer cost more than the unsafe one, which is how a safety mechanism becomes something users route around.

---

## Progress and cancellation

### T2.6 — Progress is reported, monotonic, and complete
*Covers P2.2.a, P2.2.d, P2.13.d · Verifies A-3, FR-14, FR-19, Spec §4.8*

Ingest, extract, and verify with a recording sink, over content spanning several chunks.
**Verdict:** more than one report per operation; reported positions never decrease; the final report equals the total. Ingest and extraction count bytes; verification counts entries, and so does a folder ingest — a bar that restarts at zero on every file is worse than no bar, and it is the same choice §4.8 makes for verification. A sink called once at the end satisfies "reports progress" and is useless to a progress bar, which is why monotonic growth is asserted rather than mere presence.

### T2.7 — A cancelled ingest leaves no trace in the vault
*Covers P2.2.b, P2.2.e, P2.3.d · Verifies FR-14, Spec §4.7*

Record the vault's entries, statistics, and index generation. Ingest content large enough to span several chunks, cancelling partway.
**Verdict:** the outcome is a cancellation carrying that it rolled back, not a generic failure. Entries, statistics, and generation are unchanged; the vault reopens and verifies; and every file in the directory is byte-identical to before — not merely no index trace but no bytes either. See the resolved entry on truncation below.

### T2.8 — Cancellation takes effect within a bounded number of chunks
*Covers P2.2.c · Verifies Spec §2, FR-14*

Ingest through a source that counts bytes read, cancelling after the first chunk boundary.
**Verdict:** reads stop within three chunks and well short of the source. Unbounded latency makes cancel a button that does nothing on the only files large enough for anyone to press it.

**The bound is not one chunk, and that is a property of the construction rather than slack.** Knowing which chunk is the last requires reading the next one first — STREAM tags the final chunk differently, which is what makes truncation detectable — so a hook that stops at the boundary after chunk *n* has already caused chunk *n+1* to be read. What FR-14 needs is that the bound is a constant and not the file, and that is what is asserted.

### T2.9 — A cancelled extraction removes its partial output
*Covers P2.5.d · Verifies FR-19, FR-17*

Extract a multi-chunk entry to a path, cancelling partway.
**Verdict:** the destination file does not exist. A truncated plaintext left on disk is indistinguishable from a short file, which is exactly what HC-3 forbids.

---

## Ingest

### T2.10 — Ingest is a copy
*Covers P2.3.a · Verifies FR-9, Spec §4.7*

Add a file, then a folder. Record every source file's bytes, length, and modification time beforehand.
**Verdict:** every source is present and byte-identical afterwards. Nothing in `veil-core` deletes or modifies a file outside a vault, and this is the case that would catch a "move" optimisation added later.

### T2.11 — Content is durable before the index names it
*Covers P2.3.b, P2.3.c · Verifies FR-12, Spec §4.7*

Add an entry, then inspect what the index claims against what the packs hold, and open the vault independently.
**Verdict:** the generation advanced exactly once; every extent the new entry records lies wholly inside a pack file that exists and is at least that long; an independent reader opened afterwards gets the content back byte-identically.

**This is the observable half of FR-12, and the other half is P4.2's.** The ordering of the fsyncs is not observable from outside the process, and asserting it needs the same filesystem seam the crash-injection harness needs — one decision, made once, for both. What this case establishes is that the index never points at bytes that were not written, which is the shape a wrong ordering produces, and it gives Phase 4 something to interrupt. See Open Questions.

---

## Folder ingest

### T2.13 — A folder walk stores every regular file with its relative path
*Covers P2.4.a, P2.4.e · Verifies FR-10, FR-7, HC-8*

Add a tree several levels deep containing files at every level, including a file in the root of the tree.
**Verdict:** one entry per regular file; each entry's folder is its path relative to the added root, `/`-separated on every platform; the file at the root has an empty folder rather than a `.` or a platform separator.

### T2.14 — Symbolic links are not followed and are reported
*Covers P2.4.b, P2.4.c · Verifies FR-11*

Add a tree containing a link to a file, a link to a directory inside the tree, and a link to a directory outside it.
**Verdict:** no entry corresponds to any link or to anything reachable only through one — in particular nothing from outside the tree is stored. Each link is returned to the caller as skipped, by path. Omitting them silently produces a vault the user believes is complete. *On Windows, the case is skipped when the account cannot create links, and skipping is reported rather than passing quietly.*

### T2.15 — A link cycle does not prevent the walk from finishing
*Covers P2.4.d · Verifies FR-11*

Add a tree containing a directory link pointing at one of its own ancestors.
**Verdict:** the walk terminates and stores the tree's regular files. Following links risks exactly this, which is why FR-11 forbids it; the case exists so the property is checked rather than argued.

---

## Extraction

### T2.16 — Content survives a round trip through the public API
*Covers P2.5.a · Verifies FR-16, FR-17*

Add and extract content that is empty, shorter than a chunk, exactly one chunk, one chunk plus one byte, and several chunks.
**Verdict:** byte-identical in every case. Phase 1's T1.10 proved this for the stream; this case proves it for the API a frontend actually calls, across the extent and pack machinery between them.

### T2.17 — A damaged entry produces no output file
*Covers P2.5.b, P2.5.c · Verifies FR-17, HC-3, S-4*

Flip one byte in a pack, then extract the affected entry to a path.
**Verdict:** it fails, naming the entry; the destination file does not exist afterwards; an intact entry alongside it still extracts. The original Veil left truncated plaintext in place and exited zero.

**The hash-comparison branch is not reachable from here, deliberately.** Making a recorded hash disagree with intact content means rewriting the index, which needs the index key — and whoever has it has the vault. That branch is exercised at the layer where it can be reached (T1.18, T1.19). What this case owns is the API-level obligation: on failure the entry is named and no output is left behind.

### T2.18 — Peak memory does not scale with entry size
*Covers P2.5.e · Verifies FR-20, S-1*

Ingest and extract entries of increasing size across several chunk multiples, with an allocator counting peak live bytes.
**Verdict:** peak is bounded by a small constant number of chunks and does not grow with entry size. C-2 permits 64 GiB; an implementation that buffers is unusable at the size the product exists for.

### T2.19 — Extraction writes only where the caller said
*Covers P2.5.a · Verifies HC-2, FR-16*

Extract to a `Write` that is not a file, from a working directory containing a file with the same name as the entry.
**Verdict:** no file is created anywhere; the working directory is untouched. This is a direct regression test: the original Veil wrote extraction output into the working directory, over the user's original.

---

## Replace

### T2.20 — Replace matches on the full path, never on the name alone
*Covers P2.3.f, P2.6.a · Verifies FR-13, Spec §4.6*

Store `work/2024/report.pdf` and `personal/report.pdf` with different content. Replace `work/2024/report.pdf`.
**Verdict:** exactly one entry changes, and it is the one named. `personal/report.pdf` is byte-identical to before. Matching on name alone would let an ingest into one folder silently overwrite a file in another.

### T2.21 — There is never a moment with zero intact versions
*Covers P2.6.b, P2.6.d · Verifies FR-13, HC-4*

Replace an entry with a source that fails partway through reading. Separately, replace with cancellation partway.
**Verdict:** in both, the original entry is still present and extracts byte-identically to its original content, and the vault's generation is unchanged. A remove-then-add implementation passes every test that does not fail in the middle.

### T2.22 — Replace makes the old content reclaimable in the same step
*Covers P2.6.c · Verifies FR-13, FR-21, FR-8*

Replace an entry and read the statistics.
**Verdict:** the generation advanced exactly once; reclaimable bytes grew by the old entry's stored length; the old entry is unreachable. Two generation steps would be the window HC-4 forbids.

---

## Delete

### T2.23 — A deleted entry is immediately unreachable
*Covers P2.7.a · Verifies FR-21*

Delete an entry, then enumerate and attempt to extract it.
**Verdict:** absent from the enumeration; extraction by its identifier fails as a missing entry rather than returning stale content.

### T2.24 — Delete accounts for what it did not erase
*Covers P2.7.b, P2.7.c · Verifies FR-21, FR-8, FR-29*

Delete an entry and read the statistics; inspect the pack files.
**Verdict:** reclaimable bytes grew by the entry's stored length; physical bytes did not shrink; the pack files are unchanged in size. This case asserts the honesty clause: the bytes are still there, and the figure the user is shown says so.

### T2.25 — Entry identifiers are never reused
*Covers P2.3.e · Verifies Spec §3.2, HC-3*

Add three entries, delete the last, then add another. Repeat after deleting all entries.
**Verdict:** the new identifier exceeds every identifier ever issued, including those of deleted entries and including the case where the vault is empty at the time. A reused identifier would let a wrapped key from a deleted entry decrypt under a live one's nonce — a nonce-reuse defect that no functional test would ever surface.

---

## Statistics

### T2.26 — Statistics match a full recount
*Covers P2.8.a, P2.8.c · Verifies FR-8, FR-22*

Run a fixed sequence of adds, replaces, and deletes, checking after each operation.
**Verdict:** all four totals equal an independent recount at every step. Checking only at the end lets two errors cancel; checking after each operation names the operation that diverged.

### T2.27 — Statistics are available at open without reading content
*Covers P2.1a.b, P2.8.b · Verifies FR-22, S-2*

Open a vault holding substantial content and read the statistics immediately.
**Verdict:** all four figures are correct and no pack file was opened. Deriving reclaimable space by scanning would cost more than the compaction it advises.

---

## Limits

### T2.28 — The entry limit is refused by name
*Covers P2.9.a, P2.9.d · Verifies FR-15, C-1*

With the entry limit lowered for the test, add up to it and then once more.
**Verdict:** `LimitExceeded` naming the entries-per-vault limit, the allowed value, and the actual value; the vault is unchanged and the generation did not advance. "Too many files" without the numbers leaves the user unable to act.

### T2.29 — The file-size limit is enforced against the stream, not the claim
*Covers P2.9.b, P2.9.c, P2.9.d · Verifies FR-15, C-2*

With the size limit lowered for the test, add from a source that reports a size under the limit and then yields more bytes than it claimed.
**Verdict:** `LimitExceeded` naming the file-size limit; the vault is unchanged. A limit read from file metadata is a limit on files, not on content, and every non-file source bypasses it.

---

## Password change

### T2.30 — A new password opens the vault and the old one no longer does
*Covers P2.10.a, P2.10.b · Verifies FR-4, FR-2*

Add entries, change the password, close, and reopen with each password.
**Verdict:** the new password opens and every entry extracts byte-identically; the old password gives `WrongPassword`. Content extracting unchanged is what proves the master key survived the rewrap.

### T2.31 — Password change touches only the header
*Covers P2.10.b · Verifies FR-4, A-6*

Record the bytes of every file in the vault, change the password, and compare.
**Verdict:** the header file changed; no pack file and no index slot changed. FR-4's size-independence follows from this structurally, which is a stronger statement than a timing measurement on shared CI hardware.

### T2.32 — Two changes in a row both take effect
*Covers P2.10.a · Verifies FR-4*

Change the password twice, then open with each of the three.
**Verdict:** only the newest opens; the first two give `WrongPassword`. A rewrap that reuses the salt, or that wraps under a stale KEK, passes a single-change test.

### T2.33 — A wrong old password changes nothing
*Covers P2.10.c, P2.10.d · Verifies FR-4, FR-2, HC-4*

Attempt a change supplying an incorrect current password.
**Verdict:** `WrongPassword`, the header file is byte-identical to before, and the vault still opens with the original password. Verifying after writing would destroy a vault on a typo.

---

## Integration and properties

### T2.34 — The full lifecycle runs with no terminal present
*Covers P2.11.a, P2.11.b · Verifies A-1, A-4, Spec §9*

Create, add files and a folder, browse, extract, replace, delete, read statistics, verify, change the password, and lock — all through the public API in one test.
**Verdict:** it completes. No process is spawned, no terminal is allocated, nothing is prompted. This is the phase's exit condition, and it is the single test that separates this rebuild from an original whose logic could not be exercised without a pseudo-terminal.

### T2.35 — Any byte sequence at any length survives a round trip
*Covers P2.12.a, P2.12.c · Verifies Spec §9, FR-16*

`proptest` over arbitrary content, with lengths drawn to include zero, one, and the chunk boundary and its neighbours explicitly.
**Verdict:** extraction is byte-identical for every case. The boundary lengths are generated deliberately because a uniform generator over a megabyte reaches the exact chunk length essentially never, and that is where a lookahead implementation breaks.

### T2.36 — Any sequence of operations keeps statistics true
*Covers P2.12.b · Verifies FR-22, Spec §9*

`proptest` over sequences of add, replace, and delete against a vault, comparing all four totals to a recount after each step.
**Verdict:** equal at every step. T2.26 fixes one sequence; this searches for the one that diverges.

---

## Verification

### T2.37 — Verification passes on an intact vault and writes nothing
*Covers P2.13.a · Verifies FR-33, Spec §4.8*

Record every file's bytes, verify a vault holding several entries across several packs, and compare.
**Verdict:** every entry passes and no file changed — including the index slots, since a verification that advanced a generation would make the operation a write.

### T2.38 — Verification names every failure and stops at none
*Covers P2.13.c · Verifies FR-33, S-4*

Damage two entries in different packs, leaving others intact. Separately, delete a pack file outright while the index still references it.
**Verdict:** the report lists exactly the damaged entries — not a superset, not the first casualty — and reports the intact entries as passing. The deleted pack is total damage to that pack and not a broken vault: the vault opens, its entries are enumerated, and every entry outside it verifies.

### T2.39 — A cancelled verification returns what it verified
*Covers P2.13.e · Verifies Spec §4.8, FR-14*

Cancel a verification partway through a multi-entry vault.
**Verdict:** the results for the entries completed so far are returned, marked incomplete. A partial verification is a partial answer, not a discarded one, and discarding it makes cancellation cost the user everything they waited for.

### T2.40 — Verification runs on a read-only vault
*Covers P2.13.b · Verifies Spec §4.8, FR-33*

Make a vault directory read-only and verify it.
**Verdict:** it opens and verifies. Requiring an exclusive lock, or requiring the ability to write, would make the operation that diagnoses a failing drive the one operation a failing drive cannot run. *Skipped, reporting the skip, where the test account can write regardless of permissions.*

---

## Withdrawn

**T2.12 — NFC normalisation on ingest.** Withdrawn before first use. Normalisation is Plan task P5.1 and belongs to Phase 5's test cases; Phase 2 matches on the stored form exactly, which is the identity rule of §4.6 and is covered by T2.20. The identifier is retained and not reused (G-19).

---

## Open Questions

- **Whether T2.11 justifies a filesystem indirection layer in `veil-core`.** Observing write and fsync ordering requires either a trait between the core and `std::fs` or an external tracer. The trait is testable everywhere and adds a seam to production code; the tracer is platform-specific and would make the case a Linux case in practice. P4.2's crash-injection harness needs the same seam, so the decision is shared with Phase 4 and is worth making once. **Until it is made, the fsync ordering itself is unasserted** — T2.11 covers the half that is observable and says so. Resolver: owner, before P4.2.
- **Whether the entry and file-size limits stay values the vault carries.** T2.28 and T2.29 need them lowered to run at reasonable cost, exactly as the pack cap did in Phase 1 (Phase 1 upstream note 2). They are implemented as a `Limits` value defaulting to C-1 and C-2, following that precedent; C-1 and C-2 read as product constants rather than per-vault settings, so the shape is worth confirming rather than assuming. Resolver: owner.
- **How long T2.35 and T2.36 are allowed to run.** They are configured at 24 and 32 cases, which costs roughly half a minute — chosen for a suite that runs on every push. A scheduled job could afford far more, and property tests find what they find in proportion to how long they are allowed to look. Resolver: owner, alongside the same question for the corruption matrix carried from Phase 1.
- **Carried from Phase 1, unresolved:** the Argon2id cost parameters satisfying C-3, and whether `cargo-fuzz` targets are added.

---

## Resolved since drafting

- **~~Whether a cancelled ingest should leave its unreferenced pack bytes or truncate them.~~** Truncated. `PackSink::rollback` returns the packs to their state at the start of the operation, so §4.7's "indistinguishable from one where the operation never started" holds byte-for-byte rather than only in the index. Safe because packs are append-only and the vault's exclusive lock means the bytes above the starting offset are the operation's own. Asserted by T2.7. Reconciliation (FR-32, Phase 4) is still needed for orphans a *crash* leaves, where no rollback ran.
- **~~What the lock file is called and whether it is inside the vault directory.~~** `veil.lock`, inside. The vault stays self-contained, so a copy of the directory is a copy of the vault; a sync tool replicating the file is harmless because the lock lives in the OS rather than in the file's contents. It is a §4.1 addition — see the Phase 2 to-do's upstream notes.
- **~~Whether `delete` of the last entry should reset the entry-identifier counter.~~** It does not, and the counter is now stored rather than derived: the index document carries `next_entry_id`. Deriving it from live entries reissued the identifier of a deleted entry — a nonce-reuse defect no functional test would surface — which T2.25 now covers in all three of its forms. It is a §4.3 addition; see the upstream notes.
