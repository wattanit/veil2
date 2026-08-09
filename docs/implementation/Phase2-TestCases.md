# Veil2 — Phase 2 Test Cases: Vault Operations

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-09
**Owner:** wattanit
**Foundation and plan versions these cases are built against:**
- Requirements Document v1.0 — upstream
- Design Guideline v1.0 — upstream
- Technical Specification v1.0 — upstream
- Implementation Plan v1.0 — upstream
- [Phase2-ToDo.md](Phase2-ToDo.md) v1.0 — companion; each case names the item it covers

This document owns the enumerated checks that close Phase 2. Every case cites the requirement it verifies.

**This document supersedes the previous Phase 2 test cases entirely.**

---

## Conventions

**Case identifiers** are `T<phase>.<n>`, sequential within this document.

**Every case drives the public API.** Where a case must observe something the API does not return, it observes the filesystem, never a crate-private path.

**No case requires a terminal, a prompt, or a subprocess** — that is this phase's exit condition.

**Every failure case asserts which failure.**

**Where these run.** The development machine, macOS.

---

## Lifecycle and locking

### T2.1 — A second opener is told the vault is in use
*Covers P2.1.a, P2.1.b · Verifies FR-23, Spec §2, §6*

Open a vault. With it still open, open the same directory again from the same process, then from a second process.
**Verdict:** `VaultInUse` in both, distinct from every other error.

### T2.2 — The lock does not outlive the vault, including on failure
*Covers P2.1.c · Verifies FR-23, HC-4*

Open and close a vault; reopen. Force an operation to fail mid-way, drop the vault, reopen. Open inside a scope that panics, reopen after unwinding.
**Verdict:** every reopen succeeds.

### T2.3 — Locking a vault destroys its keys
*Covers P2.1.d · Verifies FR-3, HC-2, Spec §5.1*

Call `lock` on an open vault.
**Verdict:** the value is consumed; the vault is openable again.

### T2.4 — Open reads the index and nothing else
*Covers P2.2.a, P2.2.c, P2.1.f, P2.17.f · Verifies FR-7, S-2, A-7, FR-26*

Build a vault of several entries, close it, delete every entry file, and open it again.
**Verdict:** it opens, enumerates every entry, and reports statistics identical to before — nothing that read an entry file could survive this. Separately, two vaults are open at once in one process (A-7).

### T2.5 — A vault changed on disk since open is not written over
*Covers P2.1.e · Verifies FR-24, Spec §4.3, §4.4*

Open a vault whose index slots are then replaced underneath it with a newer generation, as a sync daemon replicating another machine's write would. Attempt a write.
**Verdict:** `ChangedOnDisk` from `add`, `replace`, and `delete` alike, and the outside entry is still present afterwards.

### T2.6 — A changed vault reloads without the password
*Covers P2.1.g · Verifies FR-24*

After T2.5's refusal, reload and retry the same write.
**Verdict:** the reload adopts the outside change, and the retried write succeeds on top of it.

---

## Progress and cancellation

### T2.7 — Progress is reported, monotonic, and complete
*Covers P2.3.a, P2.3.d, P2.17.d · Verifies A-3, FR-15, FR-20, Spec §4.8*

Ingest, extract, and verify with a recording sink, over content spanning several chunks.
**Verdict:** more than one report per operation; reported positions never decrease; the final report equals the total. Ingest and extraction count bytes; verification and folder ingest count entries.

### T2.8 — A cancelled ingest leaves no trace in the vault
*Covers P2.3.e, P2.4.d · Verifies FR-15, Spec §4.7*

Record the vault's entries, statistics, and index generation. Ingest content spanning several chunks, cancelling partway.
**Verdict:** a cancellation outcome carrying that it rolled back; entries, statistics, and generation unchanged; the vault reopens and verifies; every entry file byte-identical to before.

### T2.9 — Cancellation takes effect within a bounded number of chunks
*Covers P2.3.c · Verifies Spec §2, FR-15*

Ingest through a source that counts bytes read, cancelling after the first chunk boundary.
**Verdict:** reads stop within three chunks.

### T2.10 — A cancelled extraction removes its partial output
*Covers P2.6.e · Verifies FR-19, FR-17*

Extract a multi-chunk entry to a path, cancelling partway.
**Verdict:** the destination file does not exist.

---

## Ingest

### T2.11 — Ingest is a copy
*Covers P2.4.a · Verifies FR-9, Spec §4.7*

Add a file, then a folder. Record every source file's bytes, length, and modification time beforehand.
**Verdict:** every source is present and byte-identical afterwards.

### T2.12 — Content is durable before the index names it
*Covers P2.4.b, P2.4.c · Verifies FR-12, Spec §4.7*

Add an entry, then inspect what the index claims against the entry's file on disk, and open the vault independently.
**Verdict:** the generation advanced exactly once; the entry's file exists and holds exactly the recorded length; an independent reader gets the content back byte-identically.

---

## Folder ingest

### T2.13 — A folder walk stores every regular file with its relative path
*Covers P2.5.a, P2.5.e · Verifies FR-10, FR-8*

Add a tree several levels deep containing files at every level, including a file in the root of the tree.
**Verdict:** one entry per regular file; each entry's folder is its path relative to the added root, `/`-separated on every platform; the root file has an empty folder.

### T2.14 — Symbolic links are not followed and are reported
*Covers P2.5.b, P2.5.c · Verifies FR-11*

Add a tree containing a link to a file, a link to a directory inside the tree, and a link to a directory outside it.
**Verdict:** no entry corresponds to any link or to anything reachable only through one; each link is returned to the caller as skipped, by path.

### T2.15 — A link cycle does not prevent the walk from finishing
*Covers P2.5.d · Verifies FR-11*

Add a tree containing a directory link pointing at one of its own ancestors.
**Verdict:** the walk terminates and stores the tree's regular files.

---

## Extraction

### T2.16 — Content survives a round trip through the public API
*Covers P2.6.a · Verifies FR-16, FR-17*

Add and extract content empty, shorter than a chunk, exactly one chunk, one chunk plus one byte, and several chunks.
**Verdict:** byte-identical in every case.

### T2.17 — A damaged entry produces no output file
*Covers P2.6.b, P2.6.c, P2.6.d · Verifies FR-17, HC-3, S-3*

Flip one byte in an entry's file, then extract it to a path.
**Verdict:** it fails, naming the entry; the destination file does not exist afterwards; an intact entry alongside it still extracts.

### T2.18 — Peak memory does not scale with entry size
*Covers P2.6.f · Verifies FR-20, S-1*

Ingest and extract entries of increasing size across several chunk multiples, with an allocator counting peak live bytes.
**Verdict:** peak is bounded by a small constant and does not grow with entry size.

### T2.19 — Extraction writes only where the caller said
*Covers P2.6.a · Verifies HC-2, FR-16*

Extract to a `Write` that is not a file, from a working directory containing a file with the same name as the entry.
**Verdict:** no file is created anywhere; the working directory is untouched.

---

## Replace

### T2.20 — Replace matches on the full path, never on the name alone
*Covers P2.4.f, P2.7.a · Verifies FR-13, Spec §4.6*

Store `work/2024/report.pdf` and `personal/report.pdf` with different content. Replace `work/2024/report.pdf`.
**Verdict:** exactly one entry changes; `personal/report.pdf` is byte-identical to before.

### T2.21 — There is never a moment with zero intact versions
*Covers P2.7.b, P2.7.d · Verifies FR-13, HC-4*

Replace an entry with a source that fails partway through reading. Separately, replace with cancellation partway.
**Verdict:** in both, the original entry is still present and extracts byte-identically, and the generation is unchanged.

### T2.22 — Replace advances the generation exactly once
*Covers P2.7.c · Verifies FR-13*

Replace an entry and inspect the index and the filesystem.
**Verdict:** the generation advanced exactly once; the old entry is unreachable and its file removed; the new entry extracts.

---

## Delete

### T2.23 — A deleted entry is immediately unreachable
*Covers P2.8.a · Verifies FR-22*

Delete an entry, then enumerate and attempt to extract it.
**Verdict:** absent from the enumeration; extraction by its identifier fails as a missing entry.

### T2.24 — Delete removes the entry's file immediately
*Covers P2.8.b, P2.8.c · Verifies FR-22, Spec §4.5*

Delete an entry and inspect the vault directory.
**Verdict:** the index write is present and fsynced, and the entry's file no longer exists — deletion frees the space immediately, and there is no reclaimable figure to check.

### T2.25 — Entry identifiers are never reused
*Covers P2.4.e · Verifies Spec §3.2, HC-3*

Add three entries, delete the last, then add another. Repeat after deleting all entries.
**Verdict:** the new identifier exceeds every identifier ever issued, including those of deleted entries and including the case where the vault is empty at the time.

---

## Statistics

### T2.26 — Statistics match a direct sum after any sequence of operations
*Covers P2.9.a · Verifies FR-7, FR-22*

Run a fixed sequence of adds, replaces, and deletes, checking after each operation.
**Verdict:** entry count and total size equal `entries().len()` and the sum of `entries().iter().map(|e| e.size)` at every step.

### T2.27 — Statistics are available at open without reading any entry file
*Covers P2.2.b · Verifies FR-7, S-2*

Open a vault holding substantial content and read the statistics immediately.
**Verdict:** both figures are correct and no entry file was opened.

---

## Limits

### T2.28 — The entry limit is refused by name
*Covers P2.10.a, P2.10.d · Verifies FR-16, C-1*

With the entry limit lowered for the test, add up to it and then once more.
**Verdict:** `LimitExceeded` naming the entries-per-vault limit, the allowed value, and the actual value; the vault is unchanged.

### T2.29 — The file-size limit is enforced against the stream, not the claim
*Covers P2.10.b, P2.10.c, P2.10.d · Verifies FR-16, C-2*

With the size limit lowered for the test, add from a source that reports a size under the limit and then yields more bytes than it claimed.
**Verdict:** `LimitExceeded` naming the file-size limit; the vault is unchanged.

---

## Password change

### T2.30 — A new password opens the vault and the old one no longer does
*Covers P2.11.a, P2.11.b · Verifies FR-4, FR-2*

Add entries, change the password, close, and reopen with each password.
**Verdict:** the new password opens and every entry extracts byte-identically; the old password gives `WrongPassword`.

### T2.31 — Password change touches only the header
*Covers P2.11.b · Verifies FR-4, A-6*

Record the bytes of every file in the vault, change the password, and compare.
**Verdict:** the header file changed; no entry file and no index slot changed.

### T2.32 — Two changes in a row both take effect
*Covers P2.11.a · Verifies FR-4*

Change the password twice, then open with each of the three.
**Verdict:** only the newest opens; the first two give `WrongPassword`.

### T2.33 — A wrong old password changes nothing
*Covers P2.11.c, P2.11.d · Verifies FR-4, FR-2, HC-4*

Attempt a change supplying an incorrect current password.
**Verdict:** `WrongPassword`, the header file is byte-identical to before, and the vault still opens with the original password.

---

## Integration and properties

### T2.34 — The full lifecycle runs with no terminal present
*Covers P2.15.a · Verifies A-1, A-4, Spec §9*

Create, add files and a folder, browse, extract, replace, delete, read statistics, verify, change the password, and lock — all through the public API in one test.
**Verdict:** it completes. No process is spawned, no terminal is allocated, nothing is prompted.

### T2.35 — Any byte sequence at any length survives a round trip
*Covers P2.16.a · Verifies Spec §9, FR-16*

`proptest` over arbitrary content, with lengths drawn to include zero, one, and the chunk boundary and its neighbours explicitly.
**Verdict:** extraction is byte-identical for every case.

### T2.36 — Any sequence of operations keeps statistics true
*Covers P2.16.b · Verifies FR-22, Spec §9*

`proptest` over sequences of add, replace, and delete, comparing statistics to a direct sum after each step.
**Verdict:** equal at every step.

---

## Verification

### T2.37 — Verification passes on an intact vault and writes nothing
*Covers P2.17.a · Verifies FR-26, Spec §4.8*

Record every file's bytes, verify a vault holding several entries, and compare.
**Verdict:** every entry passes and no file changed, including the index slots.

### T2.38 — Verification names every failure and stops at none
*Covers P2.17.c · Verifies FR-26, S-3*

Damage two entries' files, leaving others intact. Separately, delete an entry's file outright while the index still references it.
**Verdict:** the report lists exactly the damaged entries — not a superset, not the first casualty — and reports the intact entries as passing. The missing file is total damage to that one entry, not to the vault.

### T2.39 — A cancelled verification returns what it verified
*Covers P2.17.e · Verifies Spec §4.8, FR-15*

Cancel a verification partway through a multi-entry vault.
**Verdict:** the results for the entries completed so far are returned, marked incomplete.

### T2.40 — Verification runs on a read-only vault
*Covers P2.17.b · Verifies Spec §4.8, FR-26*

Make a vault directory read-only and verify it.
**Verdict:** it opens and verifies. *Skipped, reporting the skip, where the test account can write regardless of permissions.*

---

## Damage attribution

### T2.41 — Unreadable entries are named without reading content
*Covers P2.12.b · Verifies Spec §5.1, S-3*

Remove the files backing two entries in a vault of several, and call `unreadable_entries()`.
**Verdict:** exactly those two entries are named, and no content was read to determine it — a file-existence check per entry, nothing more.

---

## Coverage

Foundation identifiers Phase 2 verifies, and where:

| Identifier | Cases |
|---|---|
| HC-2 | T2.3, T2.19 |
| HC-3 | T2.17, T2.25 |
| HC-4 | T2.2, T2.21, T2.33 |
| A-1 | T2.34 |
| A-3 | T2.7 |
| A-6 | T2.31 |
| A-7 | T2.4 |
| FR-2 | T2.30, T2.33 |
| FR-3 | T2.3 |
| FR-4 | T2.30–T2.33 |
| FR-7 | T2.4, T2.26, T2.27 |
| FR-8 | T2.13 |
| FR-9 | T2.11 |
| FR-10, FR-11 | T2.13–T2.15 |
| FR-12 | T2.12 |
| FR-13 | T2.20–T2.22 |
| FR-15 | T2.8, T2.9 |
| FR-16 | T2.19, T2.28, T2.29, T2.35 |
| FR-17 | T2.16, T2.17 |
| FR-19 | T2.10 |
| FR-20 | T2.18 |
| FR-22 | T2.23, T2.24, T2.26, T2.36 |
| FR-23 | T2.1 |
| FR-24 | T2.5, T2.6 |
| FR-26 | T2.4, T2.37–T2.40 |
| C-1, C-2 | T2.28, T2.29 |
| S-1 | T2.18 |
| S-2 | T2.4, T2.27 |
| S-3 | T2.17, T2.38, T2.41 |

---

## Open Questions

- **Carried from Phase 1:** the Argon2id measurement against C-3.
