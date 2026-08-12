# Veil2 — Phase 4 Test Cases: Durability

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-09
**Owner:** wattanit
**Foundation and plan versions these cases are built against:**
- Requirements Document v1.0 — upstream
- Design Guideline v1.0 — upstream
- Technical Specification v1.0 — upstream
- Implementation Plan v1.0 — upstream
- [Phase4-ToDo.md](Phase4-ToDo.md) v1.0 — companion; each case names the item it covers

This document owns the enumerated checks that close Phase 4. Every case cites the requirement it verifies.

**This document supersedes the previous Phase 4 test cases entirely.** The compaction crash cases, the reconciliation-at-open cases, and the missing-pack cases are gone along with the mechanisms they tested; what remains is add, replace, delete, and the entry-file damage model.

---

## Conventions

**Case identifiers** are `T<phase>.<n>`, sequential within this document.

**The crash cases kill a real process.** No case simulates an interruption, and nothing in `veil-core` exists to let one.

**What "the vault survived" means.** Every crash case asserts:
1. the vault opens;
2. every file that existed before the killed operation is still listed;
3. each extracts byte-identically to what was stored;
4. entry count and total size match a direct sum over the resident entries.

**Where these run.** The development machine, macOS.

**How to run them.**

```bash
cargo test --release -p veil-cli                              # the crash suite, and Phase 3's
cargo test --workspace                                        # everything, debug
cargo test --release -p veil-cli --test crashes -- --ignored   # the sweep (T4.6)
```

---

## Write ordering

### T4.1 — Every write path is a known write path
*Covers P4.1.a, P4.1.b · Verifies FR-12, HC-4*

Audit `veil-core`'s source for every point at which a file is created, renamed over, or removed, and compare against an enumerated list of write paths.
**Verdict:** the set matches exactly. A new one that nobody reviewed fails the case.

---

## Crashes

### T4.2 — A kill during an add loses nothing that was already there
*Covers P4.1.c, P4.1.d, P4.2.a, P4.2.b, P4.2.d, P4.2.f · Verifies HC-4, FR-12*

Build a vault holding several files. Start adding a file large enough that the add is genuinely in flight, wait until the vault's own bytes start appearing on disk, then kill the process with an uncatchable signal.
**Verdict:** the four invariants. The interrupted file is either wholly present or wholly absent — never listed with content that does not authenticate.

### T4.3 — A kill during a replace leaves exactly one intact version
*Covers P4.1.c, P4.2.a · Verifies HC-4, FR-13*

Store a file, then replace it with different content and kill the process part-way.
**Verdict:** the path holds either the old content or the new content, in full, and it extracts. Never zero versions, never a truncated one.

### T4.4 — A kill during a delete leaves the file present or gone, never half
*Covers P4.1.c, P4.2.a · Verifies HC-4, FR-22*

Delete a file from a vault and kill the process during the operation.
**Verdict:** the four invariants, and the file is either still listed and extractable, or absent — with its file already removed from `entries/`.

### T4.5 — After any kill, the statistics are true
*Covers P4.2.c · Verifies FR-7, HC-4*

After each of T4.2 to T4.4, open the vault and compare the reported statistics against a direct sum over the resident entries.
**Verdict:** entry count and total size agree exactly. Statistics are derived on call rather than incrementally maintained, so there is nothing here that can drift — an interrupted operation either committed its index generation or it did not, and the sum reflects whichever happened.

### T4.6 — Repeated kills at unpredictable points
*Covers P4.2.e · Verifies HC-4* — `#[ignore]`, run on request

Run each operation many times, killing at a different point each run, seeded so a failure reproduces exactly.
**Verdict:** the four invariants every time. A failure names the operation and the point at which the kill landed.

### T4.7 — Both index slots are never unreadable at once
*Covers P4.2.a · Verifies HC-4, Spec §4.4*

After every kill in the suite, read both index slots directly.
**Verdict:** at least one authenticates.

---

## Residue

### T4.8 — Residue from an interrupted operation is left alone
*Covers P4.4.b, P4.4.c · Verifies HC-4, Spec §4.5*

Leave an entry file that the index does not reference beside an otherwise intact vault — the shape a replace or delete interrupted between its two steps produces — then open it, list it, and check it.
**Verdict:** opening changes nothing; the file is still there, unreferenced, afterward; listing and checking are unaffected by its presence. Nothing reads it, nothing removes it, and nothing reports it — there is no mechanism this residue is handed to.

---

## A missing entry file

### T4.9 — A missing entry file opens the vault and names its casualty
*Covers P4.3.a, P4.3.b, P4.3.d · Verifies S-3, Spec §4.5*

Remove one entry's file from a vault of several entries and open it.
**Verdict:** the vault opens; the affected entry is named without any content being read; `check` reports it in Design §7's words.

### T4.10 — Everything outside the missing entry still works
*Covers P4.3.c · Verifies S-3*

In the same vault, list, save a copy of a file stored elsewhere, and check.
**Verdict:** listing is complete, the copy is byte-identical, and `check` names exactly the one missing entry and no others.

---

## Nothing at open

### T4.11 — An open never writes
*Covers P4.4.a · Verifies HC-4, S-2, FR-24*

Open an intact vault twice, recording the generation and both index slots. Plant residue (as in T4.8) and open again.
**Verdict:** identical every time. The generation does not advance, no slot is rewritten, no entry file is touched — including on the open that coexists with residue.

---

## Read-only vaults

### T4.12 — A read-only vault opens, says so, and is not written to
*Covers P4.5.a · Verifies FR-23, Spec §4.5, §4.8*

Make a vault directory read-only and open it through the library directly. Extract a file and verify the whole vault.
**Verdict:** it opens with `Access::ReadOnly`; extraction and verification both succeed. The CLI-level refusal of a write against a read-only vault, and the exit code it carries, is T3.25's — a library-level open is what this case owns.

---

## Not covered, and why

**Power loss.** Every crash case kills a process, which proves the ordering — no index generation names bytes that had not been synced — and does not prove the platform's `fsync` reached the medium, since the page cache survives a process kill. There is no rig for whole-machine power loss.
