# Veil2 — Phase 1 To-Do: Format and Crypto Core

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-08
**Owner:** wattanit
**Foundation and plan versions this list is built against (G-14):**
- Requirements Document **v1.1** — upstream
- Design Guideline **v1.1** — upstream
- Technical Specification **v1.1** — upstream
- Implementation Plan **v1.2** — upstream; this list expands Plan tasks P1.1–P1.12

This document owns the **step-level breakdown of Phase 1**. It defers what to build to the Requirements, how it presents to the Design Guideline, how it is built to the Specification, and phase sequencing to the Implementation Plan. Enumerated checks live in [Phase1-TestCases.md](Phase1-TestCases.md).

**It is not a shadow spec (G-11).** No item below restates a format, a field, an algorithm, a chunk size, or a parameter value; each names an action and cites the section that defines what the action must produce. If implementation finds the Specification wrong or underspecified, that flows back as a Specification version bump through the feedback protocol — the candidates found so far are recorded under *Notes for Upstream* and decided by the owner, not here.

---

## Conventions

**Item identifiers** are `P<phase>.<task>.<letter>`, subdividing the Implementation Plan's task numbers. They are section-numbering references, not foundation identifiers (G-19).

**Done** for an item means the cited behaviour is observable, the test cases listed against it pass on all three platforms, and the Plan's standing definition of done holds.

---

## What Phase 1 is for

Phase 1 is the only phase whose failure cannot be corrected later. Every phase above it inherits this format and this construction, and a defect here is not a bug in a feature — it is a property of every vault ever written. That is why P1.11 gates Phase 2 rather than closing Phase 1, and why the ordering below builds the adversary's tools alongside the product's.

**Build the corruption harness early, not at P1.11.** The Plan lists the adversarial suite as one task because it is one gate; the work is not one lump. From P1.5 onward every construction gets its mutation cases written in the same sitting, so P1.11 is the moment the suite is *complete and green*, not the moment it is started. A corruption suite written after the code it attacks tends to test the code that exists rather than the requirement that was asked for.

---

## P1.1 — Argon2id key-encryption key

*Plan P1.1 · Spec §3.1, §4.2 · HC-5, HC-6, C-3*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P1.1.a | KEK derivation reading algorithm identifier, cost parameters, and salt **from the header value passed in**, with no constant available to fall back on | HC-5, Spec §3.1, §4.2 | T1.1 |
| P1.1.b | A parameter set chosen at creation time only; every later open uses what the vault recorded | HC-5 | T1.1 |
| P1.1.c | An unknown algorithm identifier is a named refusal, not a default | HC-5, HC-6, Spec §4.2 | T1.5 |
| P1.1.d | Low-cost parameters for the test profile, structurally unavailable to a release build | C-3, HC-5 | T1.35 |

**Why P1.1.a is phrased as an absence.** The original Veil hardcoded its Argon2 constants, so changing a default would have silently orphaned every existing vault. The requirement is not "read the parameters" — code that reads them and also has constants to hand will eventually use the constants. There must be nothing to fall back to.

---

## P1.2 — Master key generation and wrapping

*Plan P1.2 · Spec §3.1 · HC-5, HC-7, A-6*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P1.2.a | Master key from the OS CSPRNG at creation, never a function of the password | A-6, Spec §3.1 | T1.7 |
| P1.2.b | AEAD wrap and unwrap with the whole preceding header as associated data, so every header field is authenticated without a separate MAC | HC-3, HC-5, Spec §3.1 | T1.3 |
| P1.2.c | Unwrap failure surfaces as `WrongPassword`, distinguishably from a damaged vault, at exactly one place in the code | FR-2, Spec §6 | T1.2 |
| P1.2.d | Exactly one unwrap path in the format and in the API: no escrow slot, no second wrapping, no key export | HC-7, Spec §3.1 | T1.9 |

**On P1.2.c.** A wrong password and a tampered header both surface as an AEAD failure at the same call. They are not the same condition, and FR-2 requires the user be sent to the right remedy. Which of the two it is cannot be determined from the AEAD result alone — the distinguishing evidence is whether the header is internally consistent, so the classification must be deliberate at that one site rather than emergent from wherever the error happens to be constructed. See *Notes for Upstream*, item 1.

---

## P1.3 — Subkey derivation

*Plan P1.3 · Spec §3.1 · HC-6*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P1.3.a | HKDF-SHA256 subkeys from the master key with the versioned `info` strings the Specification fixes | HC-6, Spec §3.1 | T1.8 |
| P1.3.b | Subkeys derived once at open and held in typed values, never re-derived ad hoc at call sites | Spec §3.1 | T1.8 |
| P1.3.c | Per-entry data keys generated at ingest and wrapped under the entry-wrap subkey | Spec §3.2 | T1.10 |

**Domain separation is the point of this task.** The original Veil used one key for the header, the metadata, and every file, which is the condition that turns any single nonce mistake into total compromise. A test that only checks the subkeys differ from each other is enough to catch the regression, and it is worth having precisely because the failure it prevents is invisible until it is catastrophic.

---

## P1.4 — Header serialisation and version dispatch

*Plan P1.4 · Spec §4.2 · HC-5, FR-5, FR-30*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P1.4.a | Fixed-size header written and read with the field layout the Specification's table fixes, byte order stated once and applied everywhere | HC-5, HC-8, Spec §4.2 | T1.1, T1.30 |
| P1.4.b | Magic checked first; a mismatch is "not a Veil vault", never a corruption report | FR-2, Spec §4.2 | T1.4 |
| P1.4.c | Read dispatches on `format_version`; a newer version refuses and names both what the vault needs and what this release supports | FR-5, Spec §4.2 | T1.5 |
| P1.4.d | An older supported version opens and reports which version it is | FR-30, Spec §4.2 | T1.6 |
| P1.4.e | `writer_version` recorded on every write and never consulted in any access decision | HC-5, Spec §4.2 | T1.6 |
| P1.4.f | The header parser is total: no panic, no unbounded allocation, no hang on arbitrary bytes. It is one of the two inputs an attacker reaches before authentication | HC-3, Spec §9 | T1.33 |

**On P1.4.e.** Format version and application version have separate lifecycles precisely so that shipping a new release never invalidates a compatibility check. The way that guarantee dies is a single well-meant `if writer_version < X` somewhere, which is why the obligation is "never consulted" rather than "used carefully".

---

## P1.5 — Streaming content encryption

*Plan P1.5 · Spec §3.3 · HC-3, A-2, S-1*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P1.5.a | STREAM encryption and decryption over a `Read` to a `Write`, at the chunk size the Specification fixes | A-2, S-1, Spec §3.3 | T1.10, T1.11 |
| P1.5.b | A fresh random nonce prefix per entry, stored with the entry; no counter that code must be trusted to manage | HC-6, Spec §3.3 | T1.10 |
| P1.5.c | Entry identity bound as associated data on every chunk, so a chunk cannot be transplanted between entries | HC-3, Spec §3.3 | T1.16 |
| P1.5.d | Decryption yields no plaintext to the caller for a chunk that has not authenticated — buffering is at chunk granularity and failure discards it | HC-3, Spec §3.3 | T1.19 |
| P1.5.e | Mutation cases written alongside this task, not deferred to P1.11: bit flip, truncation at and within a chunk, reordering, transplant, and extension | HC-3, Spec §9 | T1.12–T1.17 |

**P1.5.d is the requirement the original violated.** It returned a short file and reported success. The rule is not "detect truncation" — detection that arrives after the caller has already received bytes is a report about data the user is holding. Nothing leaves this layer unauthenticated.

---

## P1.6 — Content hashing

*Plan P1.6 · Spec §3.3, §4.7 · FR-17*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P1.6.a | BLAKE3 over the plaintext computed in the same pass as encryption, so hashing costs no second read | FR-17, S-1, Spec §4.7 | T1.10 |
| P1.6.b | The hash compared after the final chunk on the read path; a mismatch fails the operation even when every chunk authenticated | FR-17, HC-3 | T1.18 |
| P1.6.c | The comparison is constant-time-free by design and stated as such — it defends against decay and index tampering, not against an adaptive oracle | FR-17 | T1.18 |

**Why P1.6.b matters when AEAD already passed.** Chunk authentication proves each chunk is what was written under this entry's key; it does not prove the index still points at the right extents or that the recorded hash was not swapped. The hash is the second, independent statement, and it is the one that survives an attacker who can rewrite the index but not forge under its key.

---

## P1.7 — Entry model and index serialisation

*Plan P1.7 · Spec §4.3 · FR-30*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P1.7.a | Entry and index document types matching the Specification's model, serialised as CBOR | Spec §4.3 | T1.30 |
| P1.7.b | Unknown fields preserved across a decode and re-encode cycle, at both document and entry level | FR-30, Spec §4.3 | T1.21 |
| P1.7.c | The whole index encrypted under the index subkey; no field of it reaches disk in the clear | HC-1, Spec §4.3 | T1.20, T1.32 |
| P1.7.d | No absolute source path stored, in any field, at any point | HC-1, Spec §4.3 | T1.20 |
| P1.7.e | The index parser is total on arbitrary decrypted bytes — authentication precedes parsing, but a damaged authenticated document must still fail rather than panic | HC-3, Spec §9 | T1.33 |

**P1.7.b is the migration door.** Requirements §2.2 defers format migration and holds the door open with HC-5 and FR-30; preserving unknown fields is the half of that door which lives in the reader. A reader that drops what it does not understand converts a future migration from a translation into a reconstruction.

---

## P1.8 — Atomic index persistence

*Plan P1.8 · Spec §4.4 · HC-4, FR-27*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P1.8.a | Two slots, each self-authenticating and carrying its generation; a write targets the older slot and fsyncs before it is considered done | HC-4, Spec §4.4 | T1.22 |
| P1.8.b | A read takes the highest generation that authenticates, falling back to the other slot rather than failing | HC-4, Spec §4.4 | T1.23 |
| P1.8.c | Both slots unusable is a loud, named failure — never a guess, never an empty index | HC-3, HC-4 | T1.24 |
| P1.8.d | The generation counter advances by exactly one per committed mutation and never repeats | FR-27, Spec §4.4 | T1.25 |
| P1.8.e | Slot corruption cases written now: damage the newer slot, damage both, damage a generation number, damage a tag | HC-3, HC-4, Spec §9 | T1.22–T1.24 |

**P1.8.b is where "the older slot is expendable" is cashed in.** The Specification chose two slots over a rename because rename atomicity varies across platforms and filesystems while slot expendability does not. That reasoning only holds if the fallback is exercised, so the fallback path is tested from the beginning rather than being the code that first runs during a real crash.

---

## P1.9 — Pack files and extents

*Plan P1.9 · Spec §4.5 · S-3, S-4, A-5, FR-25*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P1.9.a | Append-only pack write with the size cap the Specification fixes, and extent records produced as data is written | Spec §4.5 | T1.26 |
| P1.9.b | An entry larger than the cap spans packs through its extent list and reconstructs byte-identically | C-2, Spec §4.5 | T1.26 |
| P1.9.c | Reads seek directly to an extent; reading one entry touches no pack that entry has no extent in | A-5, Spec §4.5 | T1.27 |
| P1.9.d | Damage confined to a pack fails only the entries with extents in it, and the failure names them | S-4, Spec §4.5 | T1.28 |
| P1.9.e | The pack cap is injectable for tests, so spanning and multi-pack damage are exercised without gigabyte fixtures | S-4, Spec §4.5 | T1.26, T1.28 |
| P1.9.f | Writing one small entry dirties one pack and the index, and no other pack | S-3, Spec §4.5 | T1.29 |

**P1.9.e is the difference between a suite that runs and one that is skipped.** A multi-pack test that needs two gigabytes of fixture will be marked ignored within a month, and the requirements it covers — S-4's attribution and the spanning path — are among the most consequential in the format. See *Notes for Upstream*, item 2.

---

## P1.10 — Vertical slice

*Plan P1.10 · Spec §4.1–§4.5*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P1.10.a | Create a vault directory with the layout the Specification fixes; store one file; drop everything; reopen from a fresh instance and read the content back | Spec §4.1–§4.5 | T1.30 |
| P1.10.b | Confirm the slice writes nothing outside the vault directory — no temporary file, no cache, no output in the working directory | HC-2 | T1.31 |
| P1.10.c | Confirm a closed vault discloses no planted name or content anywhere in its own bytes | HC-1 | T1.32 |

**P1.10.b and P1.10.c are the two regression tests for the defects that ended the original.** `strings` over the original's metadata database returned the full index without a password, and its extraction wrote into the current working directory, overwriting a user's original with a truncated file. Both are cheap to assert and both were shipped.

---

## P1.11 — Adversarial corruption suite complete

*Plan P1.11 · Spec §9 · HC-3, S-4* — **the gate on Phase 2**

| Item | Work | Cites | Tests |
|---|---|---|---|
| P1.11.a | Every row of the Specification's corruption table has a case, and each fails as that row requires | HC-3, Spec §9 | T1.3, T1.12–T1.16, T1.28 |
| P1.11.b | Mutations applied to bytes on disk, not to in-memory structures — the attacker's position is the file, not the API | HC-3, Spec §9 | T1.12–T1.17 |
| P1.11.c | Each case asserts the *error* as well as the failure: a case satisfied by any error would pass against a build that fails on everything | HC-3, FR-2, S-4 | T1.12–T1.17, T1.28 |
| P1.11.d | Cases beyond the table recorded as such, so the trace between the suite and §9 stays legible | HC-3 | T1.17, T1.19 |

**Nothing in Phase 2 begins until this is green.** Building product on an unverified crypto core is how the original shipped silent data loss, and every later phase inherits whatever is wrong here.

---

## P1.12 — Key-derivation cost measurement

*Plan P1.12 · C-3, Spec §11.1*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P1.12.a | Measure derivation time for candidate parameter sets on the weakest supported target, not the development machine | C-3 | T1.34 |
| P1.12.b | Record the chosen values and the machine they were measured on, as the evidence resolving the Specification's open item | C-3, Spec §11.1 | T1.34 |
| P1.12.c | Confirm memory cost is satisfiable on that machine under realistic memory pressure — a vault that cannot be opened on a modest laptop is a worse failure than a slow derivation on a fast one | C-3, HC-5 | T1.34 |

**This resolves a Specification open item and therefore ends in a Specification bump**, not in a value recorded only here. The measurement is Phase 1 work; the decision it feeds is the owner's.

---

## Exit

The Implementation Plan's Phase 1 exit conditions govern. Restated as the checklist to run:

- Round-trip byte-identical for empty, single-chunk, and multi-chunk content (T1.10).
- Every row of the Specification's corruption table fails as required, including the truncated-final-chunk case the original accepted while reporting success (T1.3, T1.12–T1.16, T1.28).
- A corrupted pack fails only the entries with extents in it, and names them (T1.28).
- ~~Argon2id parameters measured on the weakest supported target and recorded (T1.34).~~ **Waived by the owner.** P1.12 is not done and no parameter set is claimed to be measured; see Open Questions.

---

## Notes for Upstream

Recorded per G-24, decided by the owner, absorbed as Specification bumps or dropped. Nothing below is decided by this document. **All three were absorbed into Specification v1.2.**

**1. Distinguishing a wrong password from a tampered header (P1.2.c).** §3.1 makes the whole header associated data, so both conditions produce one AEAD failure. §6 requires `WrongPassword` to be distinct from corruption (FR-2), but §3.1 does not say how the two are told apart at that call. A rule is needed — the plausible one is that a header failing its own internal consistency checks is corruption and an otherwise well-formed header is a wrong password — and it belongs in §3.1 rather than in an implementation habit. Resolver: owner, at the next Specification bump.

**2. Test-injectable pack size cap (P1.9.e).** §4.5 fixes the cap at 1 GiB and calls it tunable, but does not say whether it is a compile-time constant or a value the API accepts. Testing S-4's attribution and the multi-pack spanning path at realistic cost requires the latter. This is arguably already permitted by "initial; tunable"; recorded rather than assumed. Resolver: owner.

**3. Verification (§4.8) has no Phase 1 or Phase 2 presence for the hash-only case.** §4.8 reuses the extraction path, and P2.13 schedules it. Phase 1 proves the hash comparison exists (T1.18) but nothing schedules the case where a pack is intact and authenticated while the *index's* recorded hash was altered — a tampered index that still authenticates implies a compromised key, so this may be deliberately out of the threat model of Requirements §7. If it is, saying so in §7 would close the question. Resolver: owner.

---

## Open Questions

- **What Argon2id cost parameters satisfy C-3, and on what hardware.** Partly settled: the owner has accepted §11.1's estimate — `m = 256 MiB, t = 3, p = 4` — as the working value new vaults are created with, pending a low-spec machine to tune on. **Nothing has been measured against C-3's one-second budget**, so the item stays open; what closes it is the measurement, not the choice. Changing it later orphans nothing (HC-5): every vault records what it was created with, and opening reads that and never a constant. Resolver: owner, when hardware is available.
- **Whether `cargo-fuzz` targets are added for the header and index parsers.** Spec §9 calls for them. T1.33 covers the same two entry points with seeded randomised testing at lower depth; proper fuzzing needs a nightly toolchain and a tool installed on the machine. Resolver: owner.
- **~~Whether the corruption suite runs on every push or on a scheduled job.~~** Resolved: with the rest of the suite, every time `cargo test` is run. There is no scheduler and no CI (Spec §8.1).
- **~~Whether fuzz targets (P1.4.f, P1.7.e) run in CI with a time budget or only on a schedule.~~** Resolved: neither. `cargo-fuzz` is declined — it needs a nightly toolchain and a tool installed on the machine. T1.33's seeded randomised testing covers the same two parsers less deeply and runs with the suite.
</content>
