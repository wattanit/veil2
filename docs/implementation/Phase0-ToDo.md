# Veil2 — Phase 0 To-Do: Workspace and Gate Foundation

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-09
**Owner:** wattanit
**Foundation and plan versions this list is built against:**
- Requirements Document v1.0 — upstream
- Design Guideline v1.0 — upstream
- Technical Specification v1.0 — upstream
- Implementation Plan v1.0 — upstream; this list expands Plan tasks P0.1–P0.5

This document owns the step-level breakdown of Phase 0. It defers what to build to the Requirements, how it presents to the Design Guideline, how it is built to the Specification, and phase sequencing to the Implementation Plan. Enumerated checks live in [Phase0-TestCases.md](Phase0-TestCases.md).

**This list supersedes the previous Phase 0 documents entirely.** The suite it was built against is gone; no numbering or content carries forward.

---

## Conventions

**Item identifiers** are `P<phase>.<task>.<letter>`, subdividing the Implementation Plan's task numbers.

**Status** follows the Plan's convention: **built, carries forward** / **built, needs rewrite** / **built, remove entirely** / **not yet built**.

**Done** for an item means the cited behaviour is observable, the test cases listed against it pass, and the Plan's definition of done holds.

---

## Standing note

Phase 0 proves nothing about the product. Every item exists to make a later proof possible or a later defect impossible. Its own gates are already built and running; this phase's remaining work is the error-taxonomy rewrite that the storage-architecture change requires.

---

## P0.1 — Workspace and module skeleton

*Plan P0.1 · Spec §1, §8.1 · A-1, A-4*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P0.1.a | Built, carries forward | Cargo workspace, 2024 edition, shared `[workspace.dependencies]` | Spec §7, §8.1 | T0.4 |
| P0.1.b | Built, carries forward | Three member crates named as the Specification names them; `veil-core` a library, the other two binaries | Spec §1, A-4 | T0.4 |
| P0.1.c | Built, needs rewrite | `veil-core` module skeleton — the `store` module's doc comment now reads "entry file read and write" (Spec §1); the module itself is rewritten under Plan P1.10 | Spec §1 | T0.4 |
| P0.1.d | Built, carries forward | `crypto` kept free of dependencies on sibling modules, enforced by a check | Spec §1 | T0.3 |
| P0.1.e | Built, carries forward | `veil-core` takes no dependency that reads a terminal or prompts | A-1 | T0.2 |
| P0.1.f | Built, carries forward | `rustfmt.toml`, `clippy.toml`, `.gitignore`, `Cargo.lock` committed | Spec §7, §8.3 | T0.4 |

---

## P0.2 — Error taxonomy skeleton

*Plan P0.2 · Spec §6 · FR-2, FR-5, FR-6, Design §4.2*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P0.2.a | Built, carries forward | `thiserror` error enum with the variants of the Specification's taxonomy table | Spec §6 | T0.7 |
| P0.2.b | Built, carries forward | `WrongPassword` and the corruption variants distinct, never collapsing into one another | FR-2, Spec §6 | T0.7 |
| P0.2.c | Built, carries forward | `FormatTooNew` and `FormatSuperseded` carry the version numbers their messages must name | FR-5, FR-6 | T0.7 |
| P0.2.d | Built, needs rewrite | `Damaged::Pack` replaced by an entry-file-scoped damage variant, matching one-file-per-entry storage (Spec §4.5); `Unrepresentable` and `Error::NameNotRepresentable` removed entirely — no requirement supports them | Spec §6 | T0.7 |
| P0.2.e | Built, carries forward | `anyhow` absent from `veil-core`'s dependency graph, checked by a test | Spec §6 | T0.1 |
| P0.2.f | Built, carries forward | Every variant's `Display` states what happened and the resulting state | Design §4.2, Spec §6 | T0.7 |
| P0.2.g | Built, carries forward | No variant carries file content, key material, or the password in any field | HC-2, Spec §6 | T0.8 |

---

## P0.3 — Key-material types

*Plan P0.3 · Spec §3.1, §6 · HC-2*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P0.3.a | Built, carries forward | A distinct newtype per role in the key hierarchy | Spec §3.1 | T0.5 |
| P0.3.b | Built, carries forward | `ZeroizeOnDrop` on every key type and on the password type | Spec §3.1 | T0.6 |
| P0.3.c | Built, carries forward | Hand-written `Debug` printing a placeholder; no derived `Debug`, `Display`, or `Serialize` on any key type | HC-2, Spec §6 | T0.5 |
| P0.3.d | Built, carries forward | No `Clone` on key types unless a call site requires it | Spec §3.1 | — |

---

## P0.4 — Supply chain gate

*Plan P0.4 · Spec §7 · HC-6*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P0.4.a | Built, carries forward | `deny.toml` covering advisories, licences, sources, and bans; `cargo deny` and `cargo audit` fail the build | Spec §7, HC-6 | T0.4 |
| P0.4.b | Built, needs review | `veil-core`'s dependency set is otherwise pinned and matches Spec §7 — except `unicode-normalization`, which is in `Cargo.toml` and used for name normalisation (Spec §4.6) but absent from Spec §7's table | Spec §7 | T0.4 |
| P0.4.c | Built, carries forward — broader than stated | `deny.toml`'s `multiple-versions = "deny"` bans duplicate versions workspace-wide, not only for cryptographic crates, with two named, justified exceptions (`syn`, `windows-sys`) | HC-6, Spec §7 | T0.4 |

---

## P0.5 — Logging guard

*Plan P0.5 · Spec §6 · HC-1*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P0.5.a | Built, carries forward | A `tracing` capture layer for tests that records every event and its fields | Spec §6 | T0.9 |
| P0.5.b | Built, carries forward | An assertion helper that fails when a captured event contains any planted marker string, checked against fields as well as message | HC-1, Spec §6 | T0.9 |
| P0.5.c | Built, carries forward | Canary test: a call site that deliberately logs a marker, asserting the guard reports it | HC-1 | T0.9 |
| P0.5.d | Not yet built | The guard proves itself (T0.10) but is not wired into `tests/harness/mod.rs` — no vault-operation test currently runs under it. Later phases do not get it by default; each must call `support::guarded`/`support::init` itself until the harness wires it in | HC-1 | T0.10 |

---

## Exit

- Every local gate passes, and each has been seen rejecting a deliberate violation of itself.
- The canary of P0.5.c fires.
- `veil-core`'s dependency graph contains no `anyhow` and no interactive-input crate.
- `cargo deny` and `cargo audit` pass, and the lockfile is committed.

**`cargo check --workspace` does not pass yet, by design.** P0.2.d removed `Damaged::Pack` and `Unrepresentable`/`NameNotRepresentable` from `error.rs` without touching their only remaining call sites — `store/pack.rs` (Phase 1 deletes it) and `vault/representable.rs` (Phase 2 deletes it). Scoping the change to Phase 0 alone means the workspace does not build green again until those phases land. Confirmed by `cargo check --workspace --all-targets`: exactly four errors, all in those two files.

---

## Open Questions

- **`unicode-normalization` is used and pinned in `veil-core`'s `Cargo.toml` but absent from Spec §7's dependency table.** Resolver: owner, next Specification bump.
- **Whether the logging guard should be wired into `tests/harness/mod.rs` by default.** Right now it proves itself in isolation (T0.9, T0.10) but nothing outside `logging_guard.rs` invokes it, so Phase 2's real vault-operation tests do not run under it. Resolver: owner, likely at Phase 2.

None.
