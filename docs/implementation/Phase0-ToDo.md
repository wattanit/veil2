# Veil2 — Phase 0 To-Do: Workspace and CI Foundation

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-08
**Owner:** wattanit
**Foundation and plan versions this list is built against (G-14):**
- Requirements Document **v1.1** — upstream
- Design Guideline **v1.1** — upstream
- Technical Specification **v1.1** — upstream
- Implementation Plan **v1.2** — upstream; this list expands Plan tasks P0.1–P0.6

This document owns the **step-level breakdown of Phase 0**. It defers what to build to the Requirements, how it presents to the Design Guideline, how it is built to the Specification, and phase sequencing to the Implementation Plan. Enumerated checks live in [Phase0-TestCases.md](Phase0-TestCases.md).

**It is not a shadow spec (G-11).** No item below restates a format, an algorithm, or a parameter value; each names an action and cites the section that defines what the action must produce. Anything discovered here that changes *how* Veil2 is built flows back as a Technical Specification version bump, never as a correction recorded in this list.

---

## Conventions

**Item identifiers** are `P<phase>.<task>.<letter>`, subdividing the Implementation Plan's task numbers. They are section-numbering references, not foundation identifiers; the `HC`/`FR`/`A`/`C`/`S` categories are reserved for the suite (G-19).

**Done** for an item means the cited behaviour is observable, the test cases listed against it pass on all three platforms, and the Plan's standing definition of done (Plan §Conventions) holds.

---

## Standing note

**Phase 0 proves nothing about the product.** Every item exists to make a later proof possible or a later defect impossible. The one item with teeth of its own is P0.6: the logging guard is the only part of Phase 0 that can fail in a way that would matter to a user, and it is the only one with an exit condition that requires seeing it fire.

---

## P0.1 — Workspace and module skeleton

*Plan P0.1 · Spec §1, §8.1 · A-1, A-4*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P0.1.a | Workspace manifest: 2024 edition, shared `[workspace.dependencies]` so the three crates cannot drift to different versions of a cryptographic dependency | Spec §7, §8.1 | T0.1 |
| P0.1.b | Three member crates named exactly as the Specification names them; `veil-core` is a library, the other two are binaries | Spec §1, A-4 | T0.1 |
| P0.1.c | `veil-core` module skeleton — one module per row of the Specification's module table, each carrying a doc comment stating what it owns and nothing else | Spec §1 | T0.1 |
| P0.1.d | `crypto` kept free of dependencies on sibling modules, enforced by a check rather than by intent — the Specification's cheap-split property is worthless if it decays silently | Spec §1 | T0.4 |
| P0.1.e | `veil-core` takes no dependency that reads a terminal or prompts. A-1 is a property of the dependency graph, not of the code we happen to have written today | A-1 | T0.3 |
| P0.1.f | `rustfmt.toml`, `clippy.toml`, `.gitignore`, and a `Cargo.lock` committed — this is an application workspace, so the lockfile is part of the build | Spec §7, §8.3 | T0.9 |

**Note.** P0.1.d and P0.1.e are both graph checks rather than review conventions. A convention that is only checked by a human reviewer is not a constraint; it is a preference with good intentions.

---

## P0.2 — Error taxonomy skeleton

*Plan P0.2 · Spec §6 · FR-2, FR-5, FR-30, Design §4.2*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P0.2.a | `thiserror` error enum with the variants of the Specification's taxonomy table, each carrying the fields that table gives it — the fields are the requirement, not decoration | Spec §6 | T0.10 |
| P0.2.b | `WrongPassword` and the corruption variants are distinct and never collapse into one another anywhere in the crate | FR-2, Spec §6 | T0.10 |
| P0.2.c | `FormatTooNew` and `FormatSuperseded` carry the version numbers their messages must name | FR-5, FR-30 | T0.10 |
| P0.2.d | `anyhow` absent from `veil-core`'s dependency graph, checked in CI. The original Veil's `From<anyhow::Error>` conversion is exactly how a wrong password and a corrupt vault became indistinguishable, and it was one line | Spec §6 | T0.2 |
| P0.2.e | Every variant's `Display` states what happened and the state things are in, so the three-part message the Design Guideline requires can be assembled without the caller inventing facts | Design §4.2, Spec §6 | T0.10 |
| P0.2.f | No variant carries file content, key material, or the password in any field | HC-2, Spec §6 | T0.11 |

**Two prohibitions that must not be merged.** The Specification forbids content, key material, and passwords from reaching an error (§6, HC-2), and separately forbids entry names and folder metadata from reaching a *log* (§6, HC-1). Errors may therefore carry entry identity — FR-33 and S-4 require failing files to be named, and an error that cannot say which entry failed cannot satisfy them. Collapsing the two rules in either direction produces a defect: the strict reading strips errors of the facts they exist to report, the loose one writes the index into a log file. P0.2.f is the first rule; P0.6 is the second.

---

## P0.3 — Key-material types

*Plan P0.3 · Spec §3.1, §6 · HC-2*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P0.3.a | A distinct newtype per role in the key hierarchy, so a subkey cannot be passed where a master key is expected. Type confusion in a key hierarchy is a silent catastrophe, and the type system is free | Spec §3.1 | T0.5 |
| P0.3.b | `ZeroizeOnDrop` on every key type and on the password type | Spec §3.1 | T0.6 |
| P0.3.c | Hand-written `Debug` printing a placeholder; no derived `Debug`, no `Display`, no `Serialize` on any key type | HC-2, Spec §6 | T0.5 |
| P0.3.d | No `Clone` on key types unless a call site requires it, and each exception recorded where it is granted | Spec §3.1 | — |

**Honesty clause.** Zeroisation is asserted at the type level, not observed in freed memory: reading memory after a drop to confirm it is unreachable in safe Rust and not portable across the three platforms. What T0.6 proves is that every key type carries the obligation, which is what stops a later type from being added without it. It does not prove the memory was cleared. The Specification's §3.4 already declines to defend against memory capture, so nothing downstream depends on a stronger claim.

---

## P0.4 — CI matrix

*Plan P0.4 · Spec §8.1 · HC-8*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P0.4.a | Matrix over macOS, Windows, and Linux running build, test, `clippy -D warnings`, and `fmt --check` | Spec §8.1, HC-8 | T0.1, T0.9 |
| P0.4.b | Fail-fast disabled, so one platform's failure does not hide the other two. Fixing platforms one at a time is how a project acquires a primary platform and two ports | HC-8 | T0.1 |
| P0.4.c | Branch protection requiring all matrix jobs — a check that does not block a merge is a report, not a gate | Spec §8.1 | — |

---

## P0.5 — Supply chain gate

*Plan P0.5 · Spec §7 · HC-6*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P0.5.a | `deny.toml` covering advisories, licences, sources, and bans; `cargo deny` and `cargo audit` fail the build | Spec §7, HC-6 | T0.9 |
| P0.5.b | The Specification's initial dependency set added and pinned; nothing outside it added without a Specification bump | Spec §7 | T0.9 |
| P0.5.c | Ban duplicate versions of the cryptographic crates outright — two versions of an AEAD in one graph means the audited one may not be the one that runs | HC-6, Spec §7 | T0.9 |

---

## P0.6 — Logging guard

*Plan P0.6 · Spec §6 · HC-1*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P0.6.a | A `tracing` capture layer for tests that records every event and its fields | Spec §6 | T0.7 |
| P0.6.b | An assertion helper that fails when a captured event contains any planted marker string, checked against the fields as well as the message | HC-1, Spec §6 | T0.7 |
| P0.6.c | **Canary test**: a call site that deliberately logs a marker, asserting the guard reports it. This is the item — a guard nobody has watched fire is an assumption with a test name | HC-1 | T0.7 |
| P0.6.d | The guard wired into the test harness so later phases apply it to real operations by default rather than by remembering to | HC-1 | T0.8 |

**What this does and does not establish.** Phase 0 has no operations, so the guard has nothing real to guard yet. T0.7 proves the detector works; T0.8 is the standing obligation that every operation added from Phase 1 onward runs under it. The guard is a Phase 0 deliverable and a permanent cross-cutting obligation, and only the first half is finished here.

---

## Exit

The Implementation Plan's Phase 0 exit conditions govern. Restated here only as the checklist to run, not as new requirements:

- CI green on macOS, Windows, and Linux.
- The canary of P0.6.c fires: the guard detects a planted name, and the suite fails if it stops detecting it.
- `veil-core`'s dependency graph contains no `anyhow` and no interactive-input crate.
- `cargo deny` and `cargo audit` pass, and the lockfile is committed.

---

## Notes for Upstream

Recorded here rather than acted on, per the Plan's cross-cutting obligation that anything changing *how* Veil2 is built becomes a Specification bump (G-11, G-24). Nothing below is decided by this document.

**1. The Specification does not say where the logging guard lives.** §6 states the prohibition and §9 lists the test suites, but the guard is a harness component that every later phase depends on. If it belongs in the Specification's testing strategy as a named fixture rather than as an implicit consequence of §6, that is a §9 clarification. Resolver: owner, at the next Specification bump.

**2. Duplicate-version bans on cryptographic crates (P0.5.c) go beyond what §7 states.** §7 requires pinning and auditing; it does not forbid two versions of the same primitive coexisting in the graph. The ban is a stricter reading of HC-6 than the text compels. Resolver: owner — either §7 absorbs it or P0.5.c drops it.

---

## Open Questions

- **Whether CI runs on self-hosted or hosted runners for the three platforms.** Affects P0.4's cost and, later, whether P5.5's scale tests have anywhere to run. Resolver: owner, before Phase 5.
</content>
</invoke>
