# Veil2 — Phase 0 Test Cases: Workspace and Gate Foundation

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-08
**Owner:** wattanit
**Foundation and plan versions these cases are built against (G-14):**
- Requirements Document **v1.1** — upstream
- Design Guideline **v1.1** — upstream
- Technical Specification **v1.1** — upstream
- Implementation Plan **v1.2** — upstream
- [Phase0-ToDo.md](Phase0-ToDo.md) **v1.0** — companion; each case names the item it covers

This document owns the **enumerated checks that close Phase 0**. Every case cites the requirement it verifies (G-10). It defers what must be true to the Requirements and how it is built to the Specification; a case that cannot cite a foundation identifier does not belong here.

---

## Conventions

**Case identifiers** are `T<phase>.<n>` — section-numbering references, not foundation identifiers (G-19).

Every case states its **verdict condition** as an observable outcome. A case whose expected result is "looks right" is not a test case.

**Where these run.** On the development machine, macOS. No CI (Spec §8.1), so Windows and Linux are unconfirmed.

---

## Standing limitation

**Phase 0 verifies the harness, not the product.** No case below reads or writes a vault, because no vault format exists yet. What these cases establish is that the constraints later phases rely on — no interactive dependency in the core, no `anyhow` flattening, no key type without zeroisation, a logging guard that demonstrably fires — hold from the first commit rather than being retrofitted after they have already been violated somewhere. Every one of them is a constraint the original Veil violated, and each violation was cheap to prevent and expensive to discover.

---

## Structure and dependency graph

### ~~T0.1 — The workspace builds and tests on all three platforms~~ — withdrawn
*Covered the withdrawn P0.4 · Verified HC-8, A-4, Spec §1, §8.1*

**Withdrawn, and it never passed.** It needed a three-platform runner. There is none, and the workflow file was never executed, so this was reported as covered while nothing ran it. Identifier retained, not reused (G-19).

What survives is the single-platform half — the workspace builds and tests where it is developed — which T0.9 covers.

### T0.2 — `veil-core` cannot flatten its errors
*Covers P0.2.d · Verifies FR-2, Spec §6*

Inspect `veil-core`'s resolved dependency graph.
**Verdict:** `anyhow` is absent. This is a direct regression test for the original Veil, whose `From<anyhow::Error>` conversion made a wrong password and a corrupted vault the same value to every caller — the condition FR-2 exists to forbid.

### T0.3 — `veil-core` cannot prompt
*Covers P0.1.e · Verifies A-1*

Inspect `veil-core`'s resolved dependency graph against a denylist of terminal-input and prompting crates.
**Verdict:** none present. A-1 is checked as a property of the graph because the original Veil's untestability came from a dependency, not from a design document that permitted it.

### T0.4 — `crypto` depends on no sibling module
*Covers P0.1.d · Verifies Spec §1*

Check that no source file under the `crypto` module refers to `format`, `store`, `index`, or `vault`.
**Verdict:** no such reference. The Specification's claim that splitting `crypto` into its own crate stays cheap is only true while this holds, and it stops being true the first time it is violated by accident.

### T0.9 — Lint, format, and supply-chain gates reject what they are for
*Covers P0.1.f, P0.5.a, P0.5.b, P0.5.c · Verifies HC-6, Spec §7, §8.1*

Run `clippy -D warnings`, `fmt --check`, `cargo deny`, and `cargo audit`.
**Verdict:** all pass, the lockfile is committed and unchanged by the run, and each gate is confirmed to fail the build by introducing a violation of it once — an unpinned dependency for `deny`, a formatting deviation for `fmt`. A gate never observed rejecting anything is not known to be wired in.
---

## Key material

### T0.5 — No key type discloses its bytes
*Covers P0.3.a, P0.3.c · Verifies HC-2, Spec §6*

For every key type and the password type, construct a value from a distinctive byte pattern and format it with both `Debug` and, where implemented, any other formatting trait.
**Verdict:** the output contains a placeholder and no byte of the pattern, in any encoding — raw, hexadecimal, or base64. Checking only the raw bytes would pass a `Debug` that helpfully hex-dumps the key.

### T0.6 — Every key type carries the zeroisation obligation
*Covers P0.3.b · Verifies Spec §3.1*

A compile-time assertion requiring `ZeroizeOnDrop` for each key type and the password type.
**Verdict:** the assertion compiles for every type, and adding a new key type without the bound fails to compile.

*Honesty clause:* this proves the obligation is carried, not that memory was cleared. Observing freed memory is not possible in safe Rust and not portable across platforms, and the Specification's §3.4 already declines to defend against memory capture on a running machine, so nothing downstream rests on a stronger claim than this case makes.

---

## Error taxonomy

### T0.10 — The taxonomy carries the facts its consumers need
*Covers P0.2.a, P0.2.b, P0.2.c, P0.2.e · Verifies FR-2, FR-5, FR-30, FR-14, FR-15, FR-33, S-4, Design §4.2*

Construct each variant of the Specification's taxonomy table.
**Verdict:**
- `WrongPassword` and every corruption variant are distinct values, and no conversion in the crate maps one to the other.
- `FormatTooNew` and `FormatSuperseded` expose the version numbers their messages must name (FR-5, FR-30).
- `LimitExceeded` exposes both the limit and the actual value (FR-15).
- `Cancelled` exposes whether the operation rolled back (FR-14).
- `Corrupt` and `VerificationFailed` expose every affected entry rather than the first (S-4, FR-33).
- Each variant's `Display` states what happened and the resulting state, so the Design Guideline's three-part message needs no fact the caller must invent.

### T0.11 — No error discloses content, keys, or the password
*Covers P0.2.f · Verifies HC-2, Spec §6*

Construct every variant with distinctive markers planted in the surrounding state — a content marker, a key marker, a password marker — and format each with `Display` and `Debug`.
**Verdict:** no marker appears in any output.

*Scope note:* entry identity is permitted here and is not a marker. FR-33 and S-4 require failing entries to be named, so an error that cannot identify an entry cannot satisfy them. The prohibition on entry names reaching a *log* is a separate rule, tested by T0.7 and T0.8.

---

## Logging guard

### T0.7 — The guard fires
*Covers P0.6.a, P0.6.b, P0.6.c · Verifies HC-1, Spec §6*

Within the capture layer, deliberately log a distinctive marker as a message and, separately, as a structured field.
**Verdict:** the guard reports a violation in both cases. If the guard stops detecting either, this case fails.

**This is Phase 0's only exit condition with teeth.** The Implementation Plan states it as "a deliberately-added test that logs an entry name fails the build", and it is stated that way because a guard that has never been seen to fire is indistinguishable from a guard that does nothing. The failure mode it defends against — a log file that reconstructs the index — is silent, permanent, and exactly the class of defect this rebuild exists to eliminate.

### T0.8 — The guard is on by default
*Covers P0.6.d · Verifies HC-1, Spec §6*

Run the full test suite under the capture layer.
**Verdict:** no captured event contains a planted marker, and the guard is active without any test opting into it.

*Standing obligation:* Phase 0 has no vault operations, so this case currently guards an empty surface. It is re-asserted against real operations in every later phase as a cross-cutting obligation of the Implementation Plan, and it is the reason the guard is built before the first operation rather than after the first leak.

---

## Coverage

Foundation identifiers reachable in Phase 0, and where each is verified:

| Identifier | Case |
|---|---|
| HC-1 | T0.7, T0.8 |
| HC-2 | T0.5, T0.11 |
| HC-6 | T0.9 |
| HC-8 | *nothing — see the withdrawn T0.1* |
| FR-2 | T0.2, T0.10 |
| FR-5, FR-30 | T0.10 |
| FR-14, FR-15, FR-33 | T0.10 |
| A-1 | T0.3 |
| A-4 | T2.34 |
| S-4 | T0.10 |

**Not reachable in Phase 0**, and deferred to the phase that can prove them rather than tested weakly here: HC-3, HC-4, HC-5, HC-7 (Phase 1); A-2, A-3, A-5, A-6, C-1 through C-4, S-1 through S-3, and every remaining functional requirement (Phases 1–7). The error taxonomy cases above prove that a variant *carries* the facts a requirement needs, never that the behaviour behind it exists — no Phase 0 case should be read as evidence for a requirement's satisfaction.

---

## Open Questions

None. Every case above is decided; the open question about runner provisioning is recorded in the Phase 0 to-do list, where it affects scheduling rather than verdicts.
</content>
