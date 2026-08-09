# Veil2 — Phase 0 Test Cases: Workspace and Gate Foundation

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-09
**Owner:** wattanit
**Foundation and plan versions these cases are built against:**
- Requirements Document v1.0 — upstream
- Design Guideline v1.0 — upstream
- Technical Specification v1.0 — upstream
- Implementation Plan v1.0 — upstream
- [Phase0-ToDo.md](Phase0-ToDo.md) v1.0 — companion; each case names the item it covers

This document owns the enumerated checks that close Phase 0. Every case cites the requirement it verifies.

**This document supersedes the previous Phase 0 test cases entirely.**

---

## Conventions

**Case identifiers** are `T<phase>.<n>`, sequential within this document.

Every case states its verdict as an observable outcome.

**Where these run.** The development machine, macOS. Windows and Linux are not in scope (Requirements §2.1) and are not run.

---

## Structure and dependency graph

### T0.1 — `veil-core` cannot flatten its errors
*Covers P0.2.e · Verifies FR-2, Spec §6*

Inspect `veil-core`'s resolved dependency graph.
**Verdict:** `anyhow` is absent.

### T0.2 — `veil-core` cannot prompt
*Covers P0.1.e · Verifies A-1*

Inspect `veil-core`'s resolved dependency graph against a denylist of terminal-input and prompting crates.
**Verdict:** none present.

### T0.3 — `crypto` depends on no sibling module
*Covers P0.1.d · Verifies Spec §1*

Check that no source file under the `crypto` module refers to `format`, `store`, `index`, or `vault`.
**Verdict:** no such reference.

### T0.4 — Lint, format, and supply-chain gates reject what they are for
*Covers P0.1.a, P0.1.b, P0.1.c, P0.1.f, P0.4.a, P0.4.b, P0.4.c · Verifies HC-6, Spec §7, §8.1*

Run `clippy -D warnings`, `fmt --check`, `cargo deny`, and `cargo audit`.
**Verdict:** all pass, the lockfile is committed and unchanged by the run, and each gate is confirmed to fail the build by introducing a violation of it once.

---

## Key material

### T0.5 — No key type discloses its bytes
*Covers P0.3.a, P0.3.c · Verifies HC-2, Spec §6*

For every key type and the password type, construct a value from a distinctive byte pattern and format it with `Debug` and any other implemented formatting trait.
**Verdict:** the output contains a placeholder and no byte of the pattern, in any encoding.

### T0.6 — Every key type carries the zeroisation obligation
*Covers P0.3.b · Verifies Spec §3.1*

A compile-time assertion requiring `ZeroizeOnDrop` for each key type and the password type.
**Verdict:** the assertion compiles for every type; adding a new key type without the bound fails to compile.

*This proves the obligation is carried, not that memory was cleared* — observing freed memory is not possible in safe Rust, and Spec §3.4 already declines to defend against memory capture.

---

## Error taxonomy

### T0.7 — The taxonomy carries the facts its consumers need
*Covers P0.2.a, P0.2.b, P0.2.c, P0.2.d, P0.2.f · Verifies FR-2, FR-5, FR-6, FR-16, FR-15, FR-26, S-3, Design §4.2*

Construct each variant of the Specification's taxonomy table.
**Verdict:**
- `WrongPassword` and every corruption variant are distinct, and no conversion in the crate maps one to the other.
- `FormatTooNew` and `FormatSuperseded` expose the version numbers their messages must name.
- `LimitExceeded` exposes both the limit and the actual value.
- `Cancelled` exposes whether the operation rolled back.
- `Corrupt` and `VerificationFailed` expose every affected entry, never just the first.
- No variant named `Unrepresentable` or `NameNotRepresentable` exists.
- Each variant's `Display` states what happened and the resulting state.

### T0.8 — No error discloses content, keys, or the password
*Covers P0.2.g · Verifies HC-2, Spec §6*

Construct every variant with distinctive markers planted in the surrounding state, and format each with `Display` and `Debug`.
**Verdict:** no marker appears in any output.

*Entry identity is permitted and is not a marker* — S-3 requires failing entries to be named. The prohibition on entry names reaching a *log* is a separate rule, tested below.

---

## Logging guard

### T0.9 — The guard fires
*Covers P0.5.a, P0.5.b, P0.5.c · Verifies HC-1, Spec §6*

Within the capture layer, deliberately log a distinctive marker as a message and, separately, as a structured field.
**Verdict:** the guard reports a violation in both cases.

### T0.10 — The guard is on by default
*Covers P0.5.d · Verifies HC-1, Spec §6*

Run the full test suite under the capture layer.
**Verdict:** no captured event contains a planted marker, and the guard is active without any test opting in.

---

## Coverage

| Identifier | Case |
|---|---|
| HC-1 | T0.9, T0.10 |
| HC-2 | T0.5, T0.8 |
| HC-6 | T0.4 |
| FR-2 | T0.1, T0.7 |
| FR-5, FR-6 | T0.7 |
| FR-15, FR-16, FR-26 | T0.7 |
| A-1 | T0.2 |
| S-3 | T0.7 |

**Not reachable in Phase 0**, deferred to the phase that can prove them: HC-3, HC-4, HC-5, HC-7 (Phase 1); A-2, A-3, A-5, A-6, C-1 through C-4, S-1, S-2, and every remaining functional requirement (Phases 1–4).

---

## Open Questions

None.
