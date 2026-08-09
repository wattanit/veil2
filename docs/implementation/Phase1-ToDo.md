# Veil2 — Phase 1 To-Do: Format and Crypto Core

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-09
**Owner:** wattanit
**Foundation and plan versions this list is built against:**
- Requirements Document v1.0 — upstream
- Design Guideline v1.0 — upstream
- Technical Specification v1.0 — upstream
- Implementation Plan v1.0 — upstream; this list expands Plan tasks P1.1–P1.14

This document owns the step-level breakdown of Phase 1. It defers what to build to the Requirements, how it presents to the Design Guideline, how it is built to the Specification, and phase sequencing to the Implementation Plan. Enumerated checks live in [Phase1-TestCases.md](Phase1-TestCases.md).

**This list supersedes the previous Phase 1 documents entirely.** The storage architecture changed from pack files with extents to one file per entry; every item touching storage layout is rewritten rather than carried forward.

---

## Conventions

**Item identifiers** are `P<phase>.<task>.<letter>`.

**Status** follows the Plan's convention: **built, carries forward** / **built, needs rewrite** / **built, remove entirely** / **not yet built**.

**Done** for an item means the cited behaviour is observable, the test cases listed against it pass, and the Plan's definition of done holds.

---

## What Phase 1 is for

Phase 1 is the only phase whose failure cannot be corrected later. Every phase above it inherits this format and this construction. That is why P1.13's corruption suite gates Phase 2 rather than closing Phase 1.

The cryptographic construction (key hierarchy, STREAM encryption, hashing) is unaffected by the storage-architecture change and is already built. What changed is the storage layer beneath the index: one file per entry, named by id, replaces pack files and extents.

---

## P1.1 — Argon2id key-encryption key

*Plan P1.1 · Spec §3.1, §4.2 · HC-5, HC-6, C-3*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P1.1.a | Built, carries forward | KEK derivation reading algorithm identifier, cost parameters, and salt from the header value passed in, with no constant to fall back on | HC-5, Spec §3.1, §4.2 | T1.1 |
| P1.1.b | Built, carries forward | A parameter set chosen at creation time only; every later open uses what the vault recorded | HC-5 | T1.1 |
| P1.1.c | Built, carries forward | An unknown algorithm identifier is a named refusal, not a default | HC-5, HC-6, Spec §4.2 | T1.5 |
| P1.1.d | Built, carries forward | Low-cost parameters for the test profile, structurally unavailable to a release build | C-3, HC-5 | T1.34 |

---

## P1.2 — Master key generation and wrapping

*Plan P1.2 · Spec §3.1 · HC-5, HC-7, A-6*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P1.2.a | Built, carries forward | Master key from the OS CSPRNG at creation, never a function of the password | A-6, Spec §3.1 | T1.7 |
| P1.2.b | Built, carries forward | AEAD wrap and unwrap with the whole preceding header as associated data | HC-3, HC-5, Spec §3.1 | T1.3 |
| P1.2.c | Built, carries forward | Unwrap failure surfaces as `WrongPassword`, distinguishable from damage, at exactly one place in the code | FR-2, Spec §6 | T1.2 |
| P1.2.d | Built, carries forward | Exactly one unwrap path in the format and in the API — no escrow, no second wrapping, no key export | HC-7, Spec §3.1 | T1.9 |

---

## P1.3 — Subkey derivation

*Plan P1.3 · Spec §3.1 · HC-6*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P1.3.a | Built, carries forward | HKDF-SHA256 subkeys from the master key with the Specification's versioned `info` strings | HC-6, Spec §3.1 | T1.8 |
| P1.3.b | Built, carries forward | Subkeys derived once at open and held in typed values | Spec §3.1 | T1.8 |
| P1.3.c | Built, carries forward | Per-entry data keys generated at ingest, wrapped under the entry-wrap subkey | Spec §3.2 | T1.10 |

---

## P1.4 — Header serialisation and version dispatch

*Plan P1.4 · Spec §4.2 · HC-5, FR-5, FR-6*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P1.4.a | Built, carries forward | Fixed-size header, field layout per the Specification's table, byte order stated once and applied everywhere | HC-5, Spec §4.2 | T1.1 |
| P1.4.b | Built, carries forward | Magic checked first; a mismatch is "not a Veil vault", never a corruption report | FR-2, Spec §4.2 | T1.4 |
| P1.4.c | Built, carries forward | Read dispatches on `format_version`; a newer version refuses and names both the required and supported versions | FR-5, Spec §4.2 | T1.5 |
| P1.4.d | Built, carries forward | An older supported version opens and reports which version it is | FR-6, Spec §4.2 | T1.6 |
| P1.4.e | Built, carries forward | `writer_version` recorded on every write and never consulted in any access decision | HC-5, Spec §4.2 | T1.6 |
| P1.4.f | Built, carries forward | The header parser is total: no panic, no unbounded allocation, no hang on arbitrary bytes | HC-3, Spec §9 | T1.32 |

---

## P1.5 — Streaming content encryption

*Plan P1.5 · Spec §3.3 · HC-3, A-2, S-1*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P1.5.a | Built, carries forward | STREAM encryption and decryption over a `Read` to a `Write`, at the Specification's fixed chunk size | A-2, S-1, Spec §3.3 | T1.10, T1.11 |
| P1.5.b | Built, carries forward | A fresh random nonce prefix per entry, stored with the entry | HC-6, Spec §3.3 | T1.10 |
| P1.5.c | Built, carries forward | Entry identity bound as associated data on every chunk | HC-3, Spec §3.3 | T1.16 |
| P1.5.d | Built, carries forward | Decryption yields no plaintext to the caller for a chunk that has not authenticated | HC-3, Spec §3.3 | T1.19 |
| P1.5.e | Built, carries forward | Mutation cases: bit flip, truncation at and within a chunk, reordering, transplant, extension | HC-3, Spec §9 | T1.12–T1.17 |

---

## P1.6 — Content hashing

*Plan P1.6 · Spec §3.3, §4.7 · FR-18*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P1.6.a | Built, carries forward | BLAKE3 over the plaintext computed in the same pass as encryption | FR-18, S-1, Spec §4.7 | T1.10 |
| P1.6.b | Built, carries forward | The hash compared after the final chunk on the read path; a mismatch fails even when every chunk authenticated | FR-18, HC-3 | T1.18 |
| P1.6.c | Built, carries forward | The comparison defends against decay and index tampering, not an adaptive oracle, and is documented as such | FR-18 | T1.18 |

---

## P1.7 — Entry model and index serialisation

*Plan P1.7 · Spec §4.3 · FR-6*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P1.7.a | Done | Entry and index document types matching the Specification's model, serialised as CBOR — the `Extent` type and `IndexDocument.next_pack_id` are removed | Spec §4.3 | T1.20 |
| P1.7.b | Built, carries forward | Unknown fields preserved across a decode/re-encode cycle, at both document and entry level | FR-6, Spec §4.3 | T1.21 |
| P1.7.c | Built, carries forward | The whole index encrypted under the index subkey; no field reaches disk in the clear | HC-1, Spec §4.3 | T1.20, T1.31 |
| P1.7.d | Built, carries forward | No absolute source path stored, in any field | HC-1, Spec §4.3 | T1.20 |
| P1.7.e | Built, carries forward | The index parser is total on arbitrary decrypted bytes | HC-3, Spec §9 | T1.32 |

---

## P1.8 — Atomic index persistence

*Plan P1.8 · Spec §4.4 · HC-4, FR-24*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P1.8.a | Built, carries forward | Two slots, each self-authenticating and carrying its generation; a write targets the older slot and fsyncs | HC-4, Spec §4.4 | T1.22 |
| P1.8.b | Built, carries forward | A read takes the highest generation that authenticates, falling back to the other slot | HC-4, Spec §4.4 | T1.23 |
| P1.8.c | Built, carries forward | Both slots unusable is a loud, named failure | HC-3, HC-4 | T1.24 |
| P1.8.d | Built, carries forward | The generation counter advances by exactly one per committed mutation and never repeats | FR-24, Spec §4.4 | T1.25 |
| P1.8.e | Built, carries forward | Slot corruption cases: damage the newer slot, damage both, damage a generation number, damage a tag | HC-3, HC-4, Spec §9 | T1.22–T1.24 |

---

## P1.9 — Entry files

*Plan P1.9, P1.10 · Spec §4.1, §4.5 · A-5*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P1.9.a | Done | `PackSink`/`PackSource`, pack rollover, extent bookkeeping, `next_pack_id` allocation, and the pack-facing exports of `store/mod.rs` | superseded | — |
| P1.9.b | Done | One file per entry under `entries/`, named by its id (`store/entry_file.rs`: `EntryWriter`, `open_for_read`) — compiles and is unit-testable on its own; nothing in `veil-core`'s src calls it yet | Spec §4.1, §4.5 | T1.26, T1.28 |
| P1.9.c | Written, blocked on Phase 2 | Reading one entry opens only that entry's file — no other entry's file is touched. The test drives `Vault::extract` (`vault/read.rs`, Phase 2) | A-5, Spec §4.1 | T1.26 |
| P1.9.d | Written, blocked on Phase 2 | Damage to one entry's file fails only that entry, and names it — no pack-level attribution to build, since damage cannot spread past its own file. Same blocker as P1.9.c | S-3, Spec §4.5 | T1.27 |
| P1.9.e | Written, blocked on Phase 2 | Adding one entry creates exactly one new file, plus one index generation step. The test drives `Vault::add` (`vault/ingest.rs`, Phase 2) | S-3, Spec §4.5 | T1.28 |

---

## P1.10 — Name normalisation

*Plan P1.9 (folded in) · Spec §4.6*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P1.10.a | Built, carries forward | NFC normalisation on ingest | Spec §4.6 | T1.35, T1.38 |
| P1.10.b | Built, carries forward | Comparison is exact and case-sensitive after normalisation | Spec §4.6 | T1.36, T1.37 |

Already had a dedicated test file (originally labelled Phase 5, T5.1–T5.4); relabelled T1.35–T1.38 rather than left stranded under a phase that no longer exists.

---

## P1.11 — Vertical slice

*Plan P1.12 · Spec §4.1–§4.5*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P1.11.a | Written, blocked on Phase 2 | Create a vault, store one file, drop everything, reopen from a fresh instance, read the content back — exercised over the entry-file path rather than packs. Drives `Vault::add`/`extract`, so it cannot compile until Phase 2 rewrites `vault/ingest.rs` and `vault/read.rs` | Spec §4.1–§4.5 | T1.29 |
| P1.11.b | Written, blocked on Phase 2 | Confirm the slice writes nothing outside the vault directory | HC-2 | T1.30 |
| P1.11.c | Written, blocked on Phase 2 | Confirm a closed vault discloses no planted name or content anywhere in its own bytes | HC-1 | T1.31 |

---

## P1.12 — Adversarial corruption suite complete

*Plan P1.13 · Spec §9 · HC-3, S-3* — **the gate on Phase 2**

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P1.12.a | Done at the level Phase 1 owns | Every row of the Specification's corruption table has a case, and each fails as that row requires — "corrupted pack" becomes "corrupted entry file" (T1.27, driven through `Vault`, blocked on Phase 2 like T1.28–T1.29 above) | HC-3, Spec §9 | T1.3, T1.12–T1.16, T1.27 |
| P1.12.b | Built, carries forward | Mutations applied to bytes on disk, not to in-memory structures | HC-3, Spec §9 | T1.12–T1.17 |
| P1.12.c | Built, carries forward | Each case asserts the specific error, not any error | HC-3, FR-2, S-3 | T1.12–T1.17, T1.27 |

**Nothing in Phase 2 begins until this is green.**

---

## P1.13 — Key-derivation cost measurement

*Plan P1.14 · C-3, Spec §11*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P1.13.a | Partially done | Measured at 0.27s on development hardware (Spec §11); measure candidate sets on the weakest supported target | C-3 | T1.33 |
| P1.13.b | Not yet built | Record the chosen values and the machine they were measured on | C-3, Spec §11 | T1.33 |
| P1.13.c | Not yet built | Confirm memory cost is satisfiable on the weakest target under realistic memory pressure | C-3, HC-5 | T1.33 |

---

## Exit

- Round-trip byte-identical for empty, single-chunk, and multi-chunk content (T1.10).
- Every row of the Specification's corruption table fails as required, including the truncated-final-chunk case (T1.3, T1.12–T1.16).
- A corrupted entry file fails only that entry, and names it (T1.27).
- Argon2id parameters measured on the weakest supported target and recorded (T1.33).

**Not met yet, and not met by this pass:** T1.13's measurement needs a real low-end machine, which this session does not have. The exit condition stays open until that hardware is available — it is not something a code change can close.

**`cargo check --workspace` still fails, and fails in more places than after Phase 0.** This phase deleted `store/pack.rs` and its exports and removed `Extent`/`next_pack_id` from the entry model — every file in `vault/` that called the old storage API (`ingest.rs`, `read.rs`, `mutate.rs`, `session.rs`, `damage.rs`, `reclaim.rs`, plus the already-broken `representable.rs`) now fails to compile. All 38 errors from `cargo check -p veil-core --lib` are confined to `crates/veil-core/src/vault/`, i.e. Phase 2, 3, and 4's files — none in `crypto/`, `format/`, `index/`, `store/`, or `durable.rs`. Phase 1's own new test file (`tests/vault.rs`) and the rewritten `tests/index.rs` are written correctly for the new model but cannot run until Phase 2 rewrites the `vault/` call sites they exercise through the public API.

---

## Open Questions

- **What Argon2id cost parameters satisfy C-3, and on what hardware.** `m = 256 MiB, t = 3, p = 4` is the working value; nothing is measured against C-3's one-second budget on low-end hardware. Resolver: owner, when hardware is available.
- **Whether `cargo-fuzz` targets are added for the header and index parsers.** T1.32 covers the same entry points with seeded randomised testing at lower depth. Resolver: owner.
