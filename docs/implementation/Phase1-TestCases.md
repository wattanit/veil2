# Veil2 — Phase 1 Test Cases: Format and Crypto Core

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-08
**Owner:** wattanit
**Foundation and plan versions these cases are built against (G-14):**
- Requirements Document **v1.1** — upstream
- Design Guideline **v1.1** — upstream
- Technical Specification **v1.1** — upstream
- Implementation Plan **v1.2** — upstream
- [Phase1-ToDo.md](Phase1-ToDo.md) **v1.0** — companion; each case names the item it covers

This document owns the **enumerated checks that close Phase 1 and open Phase 2**. Every case cites the requirement it verifies (G-10). It defers what must be true to the Requirements and how it is built to the Specification; a case that cannot cite a foundation identifier does not belong here.

---

## Conventions

**Case identifiers** are `T<phase>.<n>` — section-numbering references, not foundation identifiers (G-19).

**Mutations are applied to bytes on disk.** Where a case corrupts, truncates, reorders, or transplants, it does so to the vault's files, not to an in-memory structure. The attacker's position is the file. A suite that mutates through the API tests the API's tolerance of its own values and proves nothing about a vault that arrives from a stolen drive.

**Every failure case asserts which failure.** "Returns an error" is satisfied by a build that fails on everything. Each case below names the error class it requires, because FR-2's whole content is that failures must be distinguishable.

**Where these run.** On the development machine. There is no CI pipeline (Spec §8.1), so nothing here says anything about Windows or Linux, and HC-8 is unverified.

---

## The gate

**T1.3 and T1.12 through T1.17 and T1.28 are the adversarial corruption suite.** Together they cover every row of the Specification's §9 corruption table. Phase 2 does not begin until all of them are green — not as a matter of tidiness, but because every phase above Phase 1 inherits this construction, and a defect at this level is a property of every vault the product will ever write.

Two of them are regression tests for defects demonstrated in the original Veil rather than hypothesised: **T1.13** (a truncated file decrypted, reported success, and exited zero) and **T1.20** and **T1.31** and **T1.32** (the index was readable with `strings` and no password, and extraction wrote into the working directory over the user's original).

---

## Header, keys, and version dispatch

### T1.1 — Key-derivation parameters come from the vault, never from the build
*Covers P1.1.a, P1.1.b, P1.4.a · Verifies HC-5, Spec §3.1, §4.2*

Create a vault with a parameter set that differs from the build's defaults in every cost field and in the salt. Reopen it with a build whose defaults have been changed again.
**Verdict:** it opens. Changing a default in a later release must never render an existing vault unopenable, and this is the case that fails if a constant is ever reachable from the derivation path.

### T1.2 — A wrong password is reported as a wrong password
*Covers P1.2.c · Verifies FR-2, Spec §6*

Open a well-formed vault with an incorrect password.
**Verdict:** `WrongPassword`, not a corruption or cryptography error. The original Veil surfaced every failure as a cryptography error, which sends a user with a typo to look for a damaged file.

### T1.3 — Tampering with any header field fails the master-key unwrap
*Covers P1.2.b, P1.4.a · Verifies HC-3, HC-5, Spec §3.1 · §9 corruption table*

Parameterised over the header fields individually: format version, KDF algorithm identifier, each cost parameter, the salt, and the wrap nonce. Alter one field on disk and open with the correct password.
**Verdict:** every case fails, and fails as corruption rather than as a wrong password. This is what makes the header need no separate MAC, and it is what closes the parameter-downgrade path — a build that passes T1.1 but fails this one will happily derive a key from an attacker's chosen cost of one.

### T1.4 — A file that is not a vault is not a damaged vault
*Covers P1.4.b · Verifies FR-2, Spec §4.2*

Point the reader at a file with the wrong magic, and at an empty file.
**Verdict:** a distinct "not a Veil vault" refusal in both cases, reported before any key derivation is attempted.

### T1.5 — A too-new format is refused by name
*Covers P1.1.c, P1.4.c · Verifies FR-5, Spec §4.2*

Set `format_version` above what the build supports; separately, set an unrecognised KDF algorithm identifier.
**Verdict:** `FormatTooNew` carrying both the version the vault requires and the version this release supports; the unknown algorithm is a named refusal and never a fallback to the default. Guessing at an unknown format is the HC-3 risk FR-5 exists to avoid.

### T1.6 — An older supported format opens, and the writer's version never gates access
*Covers P1.4.d, P1.4.e · Verifies FR-30, HC-5*

Open a vault at a supported older format version. Separately, open a vault whose `writer_version` is far ahead of and far behind the running build, at the current format version.
**Verdict:** the older format opens and reports which version it uses. The `writer_version` variations all open identically — no code path consults it for an access decision.

### T1.7 — The master key is generated, not derived
*Covers P1.2.a · Verifies A-6, Spec §3.1*

Create two vaults with the same password and store identical content in each.
**Verdict:** the wrapped master keys differ, and the stored ciphertext differs. If either matched, content keys would be a function of the password, and FR-4's size-independent password change could not exist.

### T1.8 — Subkeys are domain-separated
*Covers P1.3.a, P1.3.b · Verifies HC-6, Spec §3.1*

Derive the index subkey and the entry-wrap subkey from one master key.
**Verdict:** they differ, and neither equals the master key. The original Veil used one key for the header, the metadata, and every file — the condition that turns a single nonce mistake into total compromise.

### T1.9 — There is exactly one unwrap path
*Covers P1.2.d · Verifies HC-7, Spec §3.1*

Inspect the header layout and the public API surface.
**Verdict:** the format carries one wrapped master key and no second slot, and the API exposes no key export, escrow, or recovery entry point.

*Honesty clause:* this is a structural assertion about shape, not a proof that no recovery is possible. HC-7 is a product decision expressed in the key hierarchy, and what this case defends is that the decision cannot be undone by accident — a second wrapping added later would fail it and require a deliberate change.

---

## Content encryption

### T1.10 — Content round-trips byte-identically
*Covers P1.3.c, P1.5.a, P1.5.b, P1.6.a · Verifies HC-3, A-2, Spec §3.3*

Parameterised over sizes: empty, one byte, one byte under a chunk, exactly one chunk, one byte over a chunk, several whole chunks, several chunks plus a partial one.
**Verdict:** every size returns bytes identical to the input, and the recorded content hash matches a hash computed independently of the write path.

### T1.11 — Peak memory does not follow file size
*Covers P1.5.a · Verifies S-1, A-2*

Encrypt and decrypt inputs an order of magnitude apart in size under an allocation-tracking allocator.
**Verdict:** peak allocation differs by a bounded constant, not by the ratio of the input sizes.

*Scope note:* this establishes the shape of the curve at modest sizes. The claim at C-2's maximum is a scale test scheduled in Phase 5 (Plan P5.5), and this case is not evidence for it.

### T1.12 — A single flipped byte fails authentication
*Covers P1.5.e, P1.11.a, P1.11.b · Verifies HC-3 · §9 corruption table, row 1*

Parameterised over position: within the first chunk, within a middle chunk, within the final chunk, and within an authentication tag.
**Verdict:** every position fails, as corruption, naming the affected entry.

### T1.13 — A truncated final chunk fails
*Covers P1.5.e, P1.11.a · Verifies HC-3 · §9 corruption table, row 2*

Remove the final chunk from an entry's stored bytes and read the entry.
**Verdict:** the read fails as corruption and no output is produced.

**This is the direct regression test for the defect that ended the original.** Truncating the final chunk of a three-megabyte file there produced a two-megabyte file, a success message, and exit code zero — and because extraction wrote into the working directory, it overwrote the user's original with the truncated result. HC-3's clause that partial output is never reported as success exists because of this case, and if only one test in this document is ever run, it is this one.

### T1.14 — Truncation within a chunk fails
*Covers P1.5.e, P1.11.a · Verifies HC-3 · §9 corruption table, row 3*

Remove a partial chunk's worth of bytes from the end of an entry.
**Verdict:** fails as corruption, with no partial output.

### T1.15 — Reordered chunks fail
*Covers P1.5.e, P1.11.a · Verifies HC-3 · §9 corruption table, row 4*

Exchange two whole chunks within one entry's stored bytes.
**Verdict:** fails as corruption. Chunk position is bound by the construction, so this must fail even though every individual chunk is authentic.

### T1.16 — A chunk cannot be transplanted between entries
*Covers P1.5.c, P1.11.a · Verifies HC-3, Spec §3.3 · §9 corruption table, row 5*

Two sub-cases, and both are needed:
- **Integration.** Copy a chunk from one entry over the corresponding chunk of another and read the target entry. **Verdict:** fails as corruption.
- **Construction.** At the crypto layer, encrypt one chunk under a fixed key and nonce prefix bound to one entry identity, then attempt to decrypt it as the same position of a different entry identity. **Verdict:** fails.

*Why both:* per-entry keys make the integration case fail on the key alone, so it would pass against a build that never binds entry identity as associated data at all. The construction sub-case is the one that actually tests the Specification's claim.

### T1.17 — Extending an entry's stored bytes fails
*Covers P1.5.e, P1.11.d · Verifies HC-3*

Append a well-formed chunk, and separately arbitrary bytes, to an entry's stored data.
**Verdict:** fails as corruption.

*Trace note:* this row is not in the Specification's §9 table. It is the counterpart of truncation and is covered by HC-3's "any alteration", and it is recorded as an addition so the correspondence between the suite and §9 stays legible (P1.11.d).

### T1.18 — A content-hash mismatch fails a read that otherwise authenticated
*Covers P1.6.b, P1.6.c · Verifies FR-17, HC-3*

With the vault's keys available to the test, alter an entry's recorded content hash in the index, re-encrypt the index legitimately, and read the entry.
**Verdict:** every chunk authenticates and the read still fails, naming the entry.

*Why this is worth a case:* chunk authentication proves each chunk is what was written under this entry's key. It does not prove the index still points at the right extents or that the recorded hash was not swapped. FR-17 is the second, independent statement, and this is the only case that exercises it in isolation.

### T1.19 — No unauthenticated plaintext reaches the caller
*Covers P1.5.d, P1.11.d · Verifies HC-3*

Read an entry whose final chunk fails authentication into a sink that records everything it is handed.
**Verdict:** the sink received nothing from the failing chunk. Detection that arrives after bytes have been handed over is a report about data the user already holds, which is the shape of the original's defect rather than its fix.

---

## Index

### T1.20 — A stored index discloses nothing
*Covers P1.7.c, P1.7.d · Verifies HC-1, Spec §4.3*

Store entries whose names, folder metadata, and content contain distinctive markers. Close the vault. Search the raw bytes of both index slots for every marker, in UTF-8, UTF-16, and NFD as well as NFC.
**Verdict:** no marker appears, and no absolute source path appears in any form.

**This is the regression test for the original's defining flaw.** Running `strings` over the original Veil's metadata database with no password returned `vpath:/HR/salaries/exec_compensation_2024.csv` and the folder keys around it. Motivation 2 of the Requirements — names are secrets — exists because of that output, and this case is what keeps it fixed.

### T1.21 — Unknown fields survive a read and write cycle
*Covers P1.7.b · Verifies FR-30, Spec §4.3*

Construct an index document carrying unrecognised fields at both document and entry level. Decode it, mutate an unrelated entry, re-encode, and decode again.
**Verdict:** the unrecognised fields are present and unchanged. This is the reader's half of the migration door that Requirements §2.2 defers and HC-5 and FR-30 hold open; a reader that drops what it does not understand turns a future migration into a reconstruction.

### T1.22 — Writes alternate slots and the newest authenticating generation wins
*Covers P1.8.a, P1.8.e · Verifies HC-4, Spec §4.4*

Perform a sequence of mutations and observe which slot each write targets and which is read back.
**Verdict:** each write targets the slot holding the older generation, and each read takes the highest generation that authenticates.

### T1.23 — A damaged newer slot falls back to the previous generation
*Covers P1.8.b, P1.8.e · Verifies HC-4*

Corrupt the slot holding the higher generation — parameterised over its tag, its generation field, and its body — then open the vault.
**Verdict:** the vault opens at the previous generation with every entry that generation held, and the failure is reported rather than absorbed.

**This is where "the older slot is expendable" is cashed in.** The Specification chose two slots over a rename because slot expendability holds on every platform while rename atomicity does not. The reasoning is only worth anything if this path runs before a real crash runs it.

### T1.24 — Both slots unusable is a loud failure
*Covers P1.8.c, P1.8.e · Verifies HC-3, HC-4*

Corrupt both slots and open the vault.
**Verdict:** a named failure. Never an empty index, never a partially recovered one, never a guess.

### T1.25 — The generation counter advances by one and never repeats
*Covers P1.8.d · Verifies FR-27, Spec §4.4*

Perform a sequence of committed mutations and record the generation after each.
**Verdict:** strictly increasing, one per committed mutation. FR-27's detection of external modification is built on this counter, so a skipped or reused generation is a defect in the detector, not a cosmetic issue.

---

## Packs and extents

### T1.26 — An entry larger than the pack cap spans packs and reconstructs exactly
*Covers P1.9.a, P1.9.b, P1.9.e · Verifies C-2, A-2, Spec §4.5*

With the pack cap reduced for the test, store an entry several times the cap.
**Verdict:** it occupies multiple packs through its extent list and reads back byte-identically.

### T1.27 — Reading one entry touches no unrelated pack
*Covers P1.9.c · Verifies A-5, Spec §4.5*

Build a vault whose entries occupy several packs. Corrupt a pack in which the target entry has no extent, then read the target entry.
**Verdict:** the read succeeds. A-5 is the door held open for the mount deferral (Requirements §2.2) and the basis of the product's first motivation — retrieving one file from a several-hundred-gigabyte vault without touching the rest.

### T1.28 — Pack damage is confined and attributed
*Covers P1.9.d, P1.9.e, P1.11.a, P1.11.c · Verifies S-4, HC-3, Spec §4.5 · §9 corruption table, row 7*

With entries spread over at least three packs, corrupt one pack.
**Verdict:** entries with extents in the damaged pack fail; every other entry reads successfully; and the failure names exactly the affected entries — not a superset, not the first one, not "the vault".

**Naming them is half the requirement.** S-4 rejects two failures at once: one bad region losing everything, and one bad region being indistinguishable from total loss. A case that only asserts "some entries failed" leaves the second half untested, and the second half is what turns a partial failure into a list of files a user can go and restore from a backup.

### T1.29 — Adding one entry dirties one pack and the index
*Covers P1.9.f · Verifies S-3, Spec §4.5*

Record the content of every file in a vault of several packs. Add one small entry. Compare.
**Verdict:** exactly one pack changed, plus one index slot. S-3's acceptance standard is that adding a small file to a large vault causes a sync client to transfer megabytes rather than the vault, and this is that standard made observable at the filesystem rather than inferred from the format's design.

---

## Vault level

### T1.30 — The vertical slice round-trips across a close and reopen
*Covers P1.10.a, P1.4.a, P1.7.a · Verifies HC-3, Spec §4.1–§4.5*

Create a vault, store one file, drop every in-memory value, reopen from a fresh instance, and read the content back.
**Verdict:** identical bytes. This is the first point at which header, key hierarchy, index persistence, packs, and content encryption are proven to compose rather than to work individually.

### T1.31 — Nothing is written outside the vault directory
*Covers P1.10.b · Verifies HC-2*

Snapshot the working directory, the system temporary directory, and the user's cache locations before the slice of T1.30. Run it. Snapshot again.
**Verdict:** no new or modified file outside the vault directory.

**Regression test.** The original Veil's extraction wrote into the current working directory, which is how a truncated decryption came to overwrite the user's original. HC-2 states that no operation writes plaintext anywhere the user has not designated, and the cost of asserting it is one directory comparison.

### T1.32 — A closed vault discloses nothing
*Covers P1.10.c, P1.7.c · Verifies HC-1*

Store entries with marker names and marker content. Close the vault. Search every byte of every file in the vault directory — header, both index slots, every pack — for every marker in each encoding of T1.20.
**Verdict:** no marker appears anywhere.

*Scope note:* HC-1 accepts that total size, component count and sizes, and the fact that this is a Veil vault remain observable. This case asserts the prohibition, not the accepted disclosures, and it must not be read as claiming more than HC-1 claims.

---

## Parsers

### T1.33 — The header and index parsers are total
*Covers P1.4.f, P1.7.e · Verifies HC-3, Spec §9*

Fuzz the header parser over arbitrary bytes, and the index parser over arbitrary bytes presented as an authenticated document.
**Verdict:** no panic, no hang, no unbounded allocation. Every input is either parsed or refused.

*Why the index parser is included even though authentication precedes it:* a damaged document that still authenticates implies the key, but a panic in a parser is a defect regardless of who reaches it, and the cost of the target is one function.

---

## Key-derivation cost

### T1.34 — Derivation cost is measured on the weakest supported target
*Covers P1.12.a, P1.12.b, P1.12.c · Verifies C-3, Spec §11.1*

Measure candidate parameter sets on the least capable machine in the supported range, under realistic memory pressure. Record the values, the machine, and the measurements.
**Verdict:** the chosen set approaches C-3's one-second budget on that machine and its memory cost is satisfiable there.

*This is a measurement, not a pass or fail assertion.* Timing assertions are noise on a machine doing other work. The output is evidence that resolves the Specification's open item, and the decision it feeds is the owner's.

### T1.35 — Test parameters cannot reach a release build
*Covers P1.1.d · Verifies C-3, HC-5*

Attempt to select the low-cost test parameter set from a release build.
**Verdict:** it is unavailable. A vault created with test parameters is a weak vault, and the parameters are recorded in it permanently (HC-5), so a leak here is not a slow build — it is a vault that stays weak for its whole life.

---

## Coverage

Foundation identifiers Phase 1 verifies, and where:

| Identifier | Cases |
|---|---|
| HC-1 | T1.20, T1.32 |
| HC-2 | T1.31 |
| HC-3 | T1.3, T1.10, T1.12–T1.19, T1.24, T1.28, T1.30, T1.33 |
| HC-4 | T1.22, T1.23, T1.24 |
| HC-5 | T1.1, T1.3, T1.6, T1.35 |
| HC-6 | T1.8 |
| HC-7 | T1.9 |
| HC-8 | every case, by running on all three platforms |
| FR-2 | T1.2, T1.4 |
| FR-5 | T1.5 |
| FR-17 | T1.18 |
| FR-27 | T1.25 |
| FR-30 | T1.6, T1.21 |
| A-2 | T1.10, T1.11, T1.26 |
| A-5 | T1.27 |
| A-6 | T1.7 |
| C-2 | T1.26 |
| C-3 | T1.34, T1.35 |
| S-1 | T1.11 |
| S-3 | T1.29 |
| S-4 | T1.28 |

### What Phase 1 does not prove

Stated so that a green Phase 1 is not read as more than it is:

| Not proven here | Proven in |
|---|---|
| HC-4 against real interruption at every fsync boundary — T1.22–T1.24 simulate damage, they do not crash | Phase 4 (Plan P4.2, P4.6) |
| A missing-but-referenced pack opening the vault and enumerating its casualties (S-4) | Phase 4 (Plan P4.5) |
| Compaction, reconciliation, and bounded working space (FR-23, FR-24, FR-25, FR-32) | Phase 4 |
| Name normalisation, case handling, and representability (HC-8, FR-13, FR-31) | Phase 5 (Plan P5.1–P5.3) |
| S-1 and S-2 at the sizes of C-1 and C-2 | Phase 5 (Plan P5.5) |
| Ingest and extraction semantics — copy, symlinks, cancellation, overwrite confirmation, partial-output removal (FR-9 through FR-20) | Phase 2 |
| Statistics, limits, password change, verification (FR-4, FR-8, FR-15, FR-22, FR-33) | Phase 2 |
| The logging guard applied to real operations (HC-1) | every phase, as a cross-cutting obligation |

**A-1 and A-3 are not tested in Phase 1 and are not deferred either** — they are properties of how the code above is written, asserted structurally in Phase 0 (T0.3) and exercised in Phase 2 when the first long-running operation acquires a progress sink and a cancellation token.

---

## Open Questions

- **Whether T1.12's parameterisation runs every position on every push or a sampled subset with the full matrix on a schedule.** The suite is the gate on Phase 2, so it runs before any merge touching `veil-core`; the breadth of the parameterisation is a cost decision. Resolver: owner, at P1.11.
- **Whether T1.34's target machine is a specific piece of hardware or a constrained container.** A container bounds memory convincingly but not single-core speed, and C-3's budget is about both. Resolver: owner, at P1.12.
</content>
