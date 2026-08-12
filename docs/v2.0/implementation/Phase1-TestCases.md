# Veil2 — Phase 1 Test Cases: Format and Crypto Core

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-09
**Owner:** wattanit
**Foundation and plan versions these cases are built against:**
- Requirements Document v1.0 — upstream
- Design Guideline v1.0 — upstream
- Technical Specification v1.0 — upstream
- Implementation Plan v1.0 — upstream
- [Phase1-ToDo.md](Phase1-ToDo.md) v1.0 — companion; each case names the item it covers

This document owns the enumerated checks that close Phase 1 and open Phase 2. Every case cites the requirement it verifies.

**This document supersedes the previous Phase 1 test cases entirely.**

---

## Conventions

**Case identifiers** are `T<phase>.<n>`, sequential within this document.

**Mutations are applied to bytes on disk.** The attacker's position is the file, not the API.

**Every failure case asserts which failure.** "Returns an error" is satisfied by a build that fails on everything.

**Where these run.** The development machine, macOS.

---

## The gate

**T1.3, T1.12 through T1.17, and T1.27 are the adversarial corruption suite.** Together they cover every row of the Specification's §9 corruption table. Phase 2 does not begin until all of them are green.

Two are direct regression tests for defects demonstrated in the original Veil: **T1.13** (a truncated file decrypted, reported success, and exited zero) and **T1.20**/**T1.30**/**T1.31** (the index was readable with `strings` and no password, and extraction wrote into the working directory over the user's original).

---

## Header, keys, and version dispatch

### T1.1 — Key-derivation parameters come from the vault, never from the build
*Covers P1.1.a, P1.1.b, P1.4.a · Verifies HC-5, Spec §3.1, §4.2*

Create a vault with a parameter set that differs from the build's defaults in every cost field and in the salt. Reopen with a build whose defaults have changed again.
**Verdict:** it opens.

### T1.2 — A wrong password is reported as a wrong password
*Covers P1.2.c · Verifies FR-2, Spec §6*

Open a well-formed vault with an incorrect password.
**Verdict:** `WrongPassword`, not a corruption or cryptography error.

### T1.3 — Tampering with any header field fails the master-key unwrap
*Covers P1.2.b, P1.4.a · Verifies HC-3, HC-5, Spec §3.1 · §9 corruption table*

Parameterised over the header fields individually. Alter one field on disk, open with the correct password.
**Verdict:** every case fails, and fails as corruption rather than as a wrong password.

### T1.4 — A file that is not a vault is not a damaged vault
*Covers P1.4.b · Verifies FR-2, Spec §4.2*

Point the reader at a file with the wrong magic, and at an empty file.
**Verdict:** a distinct "not a Veil vault" refusal in both cases, before any key derivation is attempted.

### T1.5 — A too-new format is refused by name
*Covers P1.1.c, P1.4.c · Verifies FR-5, Spec §4.2*

Set `format_version` above what the build supports; separately, set an unrecognised KDF algorithm identifier.
**Verdict:** `FormatTooNew` carrying both versions; the unknown algorithm is a named refusal, never a fallback.

### T1.6 — An older supported format opens, and the writer's version never gates access
*Covers P1.4.d, P1.4.e · Verifies FR-6, HC-5*

Open a vault at a supported older format version. Separately, open a vault whose `writer_version` is far ahead of and far behind the running build, at the current format version.
**Verdict:** the older format opens and reports which version it uses; the `writer_version` variations all open identically.

### T1.7 — The master key is generated, not derived
*Covers P1.2.a · Verifies A-6, Spec §3.1*

Create two vaults with the same password and identical content.
**Verdict:** the wrapped master keys differ, and the stored ciphertext differs.

### T1.8 — Subkeys are domain-separated
*Covers P1.3.a, P1.3.b · Verifies HC-6, Spec §3.1*

Derive the index subkey and the entry-wrap subkey from one master key.
**Verdict:** they differ, and neither equals the master key.

### T1.9 — There is exactly one unwrap path
*Covers P1.2.d · Verifies HC-7, Spec §3.1*

Inspect the header layout and the public API surface.
**Verdict:** the format carries one wrapped master key and no second slot; the API exposes no key export, escrow, or recovery entry point.

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

### T1.12 — A single flipped byte fails authentication
*Covers P1.5.e, P1.12.a, P1.12.b · Verifies HC-3 · §9 corruption table, row 1*

Parameterised over position: within the first chunk, a middle chunk, the final chunk, and an authentication tag.
**Verdict:** every position fails, as corruption, naming the affected entry.

### T1.13 — A truncated final chunk fails
*Covers P1.5.e, P1.12.a · Verifies HC-3 · §9 corruption table, row 2*

Remove the final chunk from an entry's stored bytes and read the entry.
**Verdict:** the read fails as corruption and no output is produced. This is the direct regression test for the defect that ended the original Veil: a truncated file there decrypted, reported success, and exited zero.

### T1.14 — Truncation within a chunk fails
*Covers P1.5.e, P1.12.a · Verifies HC-3 · §9 corruption table, row 3*

Remove a partial chunk's worth of bytes from the end of an entry.
**Verdict:** fails as corruption, with no partial output.

### T1.15 — Reordered chunks fail
*Covers P1.5.e, P1.12.a · Verifies HC-3 · §9 corruption table, row 4*

Exchange two whole chunks within one entry's stored bytes.
**Verdict:** fails as corruption.

### T1.16 — A chunk cannot be transplanted between entries
*Covers P1.5.c, P1.12.a · Verifies HC-3, Spec §3.3 · §9 corruption table, row 5*

- **Integration.** Copy a chunk from one entry over the corresponding chunk of another and read the target. **Verdict:** fails as corruption.
- **Construction.** At the crypto layer, encrypt one chunk under a fixed key and nonce prefix bound to one entry identity, then decrypt it as the same position of a different entry identity. **Verdict:** fails.

Per-entry keys make the integration case fail on the key alone; the construction case is the one that actually tests entry-identity binding.

### T1.17 — Extending an entry's stored bytes fails
*Covers P1.5.e · Verifies HC-3*

Append a well-formed chunk, and separately arbitrary bytes, to an entry's stored data.
**Verdict:** fails as corruption. Not a row of the §9 table; covered by HC-3's "any alteration."

### T1.18 — A content-hash mismatch fails a read that otherwise authenticated
*Covers P1.6.b, P1.6.c · Verifies FR-18, HC-3*

With the vault's keys available to the test, alter an entry's recorded content hash in the index, re-encrypt the index legitimately, and read the entry.
**Verdict:** every chunk authenticates and the read still fails, naming the entry.

### T1.19 — No unauthenticated plaintext reaches the caller
*Covers P1.5.d · Verifies HC-3*

Read an entry whose final chunk fails authentication into a sink that records everything it is handed.
**Verdict:** the sink received nothing from the failing chunk.

---

## Index

### T1.20 — A stored index discloses nothing
*Covers P1.7.c, P1.7.d · Verifies HC-1, Spec §4.3*

Store entries whose names, folder metadata, and content contain distinctive markers. Close the vault. Search the raw bytes of both index slots for every marker, in UTF-8, UTF-16, and NFD as well as NFC.
**Verdict:** no marker appears, and no absolute source path appears in any form. Direct regression test: `strings` over the original Veil's metadata database with no password returned its file paths in plain text.

### T1.21 — Unknown fields survive a read and write cycle
*Covers P1.7.b · Verifies FR-6, Spec §4.3*

Construct an index document carrying unrecognised fields at both document and entry level. Decode, mutate an unrelated entry, re-encode, decode again.
**Verdict:** the unrecognised fields are present and unchanged.

### T1.22 — Writes alternate slots and the newest authenticating generation wins
*Covers P1.8.a, P1.8.e · Verifies HC-4, Spec §4.4*

Perform a sequence of mutations and observe which slot each write targets and which is read back.
**Verdict:** each write targets the slot holding the older generation; each read takes the highest generation that authenticates.

### T1.23 — A damaged newer slot falls back to the previous generation
*Covers P1.8.b, P1.8.e · Verifies HC-4*

Corrupt the slot holding the higher generation — its tag, its generation field, and its body, in turn — then open.
**Verdict:** the vault opens at the previous generation with every entry it held, and the failure is reported rather than absorbed.

### T1.24 — Both slots unusable is a loud failure
*Covers P1.8.c, P1.8.e · Verifies HC-3, HC-4*

Corrupt both slots and open the vault.
**Verdict:** a named failure. Never an empty index, never a partially recovered one.

### T1.25 — The generation counter advances by one and never repeats
*Covers P1.8.d · Verifies FR-24, Spec §4.4*

Perform a sequence of committed mutations and record the generation after each.
**Verdict:** strictly increasing, one per committed mutation.

---

## Entry files

**Status.** `store/entry_file.rs` and the entry model exist and compile. T1.26 through T1.31 below all drive `Vault::add`/`Vault::extract`, which still call the deleted pack API in `vault/ingest.rs` and `vault/read.rs` — these cases are written correctly for the new model but cannot compile, let alone pass, until Phase 2 rewrites those two files.

### T1.26 — Reading one entry touches no other entry's file
*Covers P1.9.c · Verifies A-5, Spec §4.1*

Build a vault of several entries. Delete the file backing an entry other than the one being read, then read the target entry.
**Verdict:** the read succeeds — reading one entry never opens another entry's file. This is A-5's structural claim, and the basis of the product's first motivation: retrieving one file from a large vault without touching the rest.

### T1.27 — A corrupted entry file fails only that entry
*Covers P1.9.d, P1.12.a, P1.12.c · Verifies S-3, HC-3, Spec §4.5 · §9 corruption table, row 7*

Corrupt one entry's file in a vault holding several entries.
**Verdict:** the damaged entry fails, naming itself; every other entry reads successfully. With one file per entry, damage cannot spread past its own file — there is no pack-level attribution to assert, only the per-entry failure and the fact that it is confined.

### T1.28 — Adding one entry writes exactly one new file
*Covers P1.9.b, P1.9.e · Verifies S-3, Spec §4.5*

Record every file present in a vault's directory. Add one entry.
**Verdict:** exactly one new file under `entries/`, plus one index slot changed. No other entry's file is touched.

---

## Vault level

### T1.29 — The vertical slice round-trips across a close and reopen
*Covers P1.11.a, P1.4.a, P1.7.a · Verifies HC-3, Spec §4.1–§4.5*

Create a vault, store one file, drop every in-memory value, reopen from a fresh instance, read the content back.
**Verdict:** identical bytes. First point at which header, key hierarchy, index persistence, entry-file storage, and content encryption are proven to compose.

### T1.30 — Nothing is written outside the vault directory
*Covers P1.11.b · Verifies HC-2*

Snapshot the working directory, the system temporary directory, and the user's cache locations before the slice of T1.29. Run it. Snapshot again.
**Verdict:** no new or modified file outside the vault directory. Regression test: the original Veil's extraction wrote into the current working directory.

### T1.31 — A closed vault discloses nothing
*Covers P1.11.c, P1.7.c · Verifies HC-1*

Store entries with marker names and marker content. Close the vault. Search every byte of every file in the vault directory — header, both index slots, every entry file — for every marker in each encoding of T1.20.
**Verdict:** no marker appears anywhere.

*HC-1 accepts that total size, component count and sizes, and the fact that this is a Veil vault remain observable — this case asserts the prohibition, not more than HC-1 claims.*

---

## Parsers

### T1.32 — The header and index parsers are total
*Covers P1.4.f, P1.7.e · Verifies HC-3, Spec §9*

Fuzz the header parser over arbitrary bytes, and the index parser over arbitrary bytes presented as an authenticated document.
**Verdict:** no panic, no hang, no unbounded allocation. Every input is either parsed or refused.

---

## Key-derivation cost

### T1.33 — Derivation cost is measured on the weakest supported target
*Covers P1.13.a, P1.13.b, P1.13.c · Verifies C-3, Spec §11*

Measure candidate parameter sets on the least capable machine in the supported range, under realistic memory pressure. Record the values, the machine, and the measurements.
**Verdict:** the chosen set approaches C-3's one-second budget on that machine and its memory cost is satisfiable there.

### T1.34 — Test parameters cannot reach a release build
*Covers P1.1.d · Verifies C-3, HC-5*

Attempt to select the low-cost test parameter set from a release build.
**Verdict:** it is unavailable.

---

## Name normalisation

*Not new work — these four cases already existed, originally labelled Phase 5 (T5.1–T5.4). Relabelled here rather than left stranded under a phase that no longer exists. They drive `Vault::add`/`add_path`/`add_folder`/`replace`/`find` through the Phase 2 shared harness, so — like the entry-file cases above — they are written correctly but blocked on Phase 2.*

### T1.35 — An NFD name and its NFC spelling store as one entry
*Covers P1.10.a · Verifies Spec §4.6*

Add the same name through a literal NFC spelling, a literal NFD spelling, and a source file whose on-disk name is NFD.
**Verdict:** all three store as the identical NFC name.

### T1.36 — Matching a stored name by its other spelling
*Covers P1.10.b · Verifies Spec §4.6, FR-13*

Store a name in NFC, then `find` and `replace` it using its NFD spelling.
**Verdict:** both resolve to the same entry; replace does not insert a second one.

### T1.37 — Case sensitivity is unaffected by normalisation
*Covers P1.10.b · Verifies Spec §4.6*

Add two names differing only by case.
**Verdict:** both are stored as distinct entries.

### T1.38 — A folder walk over NFD-yielding paths produces NFC folder metadata
*Covers P1.10.a · Verifies Spec §4.6, FR-10*

Walk a folder whose on-disk folder segment and file name are both NFD.
**Verdict:** both the stored name and the stored folder segment are NFC.

---

## Coverage

| Identifier | Cases |
|---|---|
| HC-1 | T1.20, T1.31 |
| HC-2 | T1.30 |
| HC-3 | T1.3, T1.10, T1.12–T1.19, T1.24, T1.27, T1.29, T1.32 |
| HC-4 | T1.22, T1.23, T1.24 |
| HC-5 | T1.1, T1.3, T1.6, T1.34 |
| HC-6 | T1.8 |
| HC-7 | T1.9 |
| FR-2 | T1.2, T1.4 |
| FR-5 | T1.5 |
| FR-6 | T1.6, T1.21 |
| FR-10 | T1.38 |
| FR-13 | T1.36 |
| FR-18 | T1.18 |
| FR-24 | T1.25 |
| A-2 | T1.10, T1.11 |
| A-5 | T1.26 |
| A-6 | T1.7 |
| C-3 | T1.33, T1.34 |
| S-1 | T1.11 |
| S-3 | T1.27, T1.28 |

**Blocked on Phase 2, and reachable only once it lands:** T1.26 through T1.31, T1.35 through T1.38 — every case that drives `Vault::add` or `Vault::extract`.

### What Phase 1 does not prove

| Not proven here | Proven in |
|---|---|
| HC-4 against real interruption at every fsync boundary — T1.22–T1.24 simulate damage, they do not crash | Phase 4 |
| Ingest and extraction semantics — copy, symlinks, cancellation, overwrite confirmation, partial-output removal | Phase 2 |
| Statistics, limits, password change, verification | Phase 2 |
| S-1 and S-2 at C-1/C-2 scale | Phase 2 (scale cases) |
| The logging guard applied to real operations | every phase, cross-cutting |

**A-1 and A-3 are properties of how the code is written**, asserted structurally in Phase 0 and exercised in Phase 2 when the first long-running operation acquires a progress sink and cancellation token.

---

## Open Questions

- **Whether T1.12's parameterisation runs every position on every push or a sampled subset.** Resolver: owner, at P1.12.
- **Whether T1.33's target machine is specific hardware or a constrained container.** Resolver: owner, at P1.13.
