# Veil2 — Phase 3 To-Do: Command-Line Application

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-08
**Owner:** wattanit
**Foundation and plan versions this list is built against (G-14):**
- Requirements Document **v1.2** — upstream
- Design Guideline **v1.2** — upstream
- Technical Specification **v1.3** — upstream
- Implementation Plan **v1.5** — upstream; this list expands Plan tasks P3.1–P3.7

This document owns the **step-level breakdown of Phase 3**. It defers what to build to the Requirements, how it presents to the Design Guideline, how it is built to the Specification, and phase sequencing to the Implementation Plan. Enumerated checks live in [Phase3-TestCases.md](Phase3-TestCases.md).

**It is not a shadow spec (G-11).** No item below restates a format, an algorithm, or a parameter value. Where this phase must fix something the foundation documents leave open — the exit-code numbers, the column order, the words the commands are spelled with — it is recorded under *Notes for Upstream* and decided by the owner, not settled here.

---

## Conventions

**Item identifiers** are `P<phase>.<task>.<letter>`, subdividing the Implementation Plan's task numbers. They are section-numbering references, not foundation identifiers (G-19).

**Done** for an item means the cited behaviour is observable from a shell, and the test cases listed against it pass.

---

## What Phase 3 is for

Phase 2 built an API with no caller. Phase 3 is the caller, and the Plan is explicit about why it comes before durability work: crash-injection through a command is cheap, through a UI it is not. Everything Phase 4 does to prove HC-4 will be driven through the binary this phase produces.

**Parity is the requirement, not a goal** (A-4). The CLI is a peer of the GUI, so a capability reachable only from one of them is a defect in this phase. Parity is measured against the core *as it exists*: compaction is not in `veil-core` until P4.3, so the command that reclaims space arrives with it, in Phase 4, not as a stub here. That is stated so "Phase 3 is done" cannot be read as "reclaiming space was considered and left out".

**The user-facing words are fixed by Design §7 and are not negotiable per-command.** The vocabulary table is a product decision: the CLI says *file*, *folder*, *add*, *save a copy*, *lock*, *check for damage*. It never says entry, export, extract, decrypt, verify, or compact — those are the words in this repository's source and in these documents, and they stop at the process boundary.

**What this phase deliberately does not do.** No daemon, no watch mode, no config file, no shell completions, no colour theming. Each is a thing a CLI commonly grows; none is required by any FR, and the first two are FR-23's prohibition wearing a different hat.

---

## P3.1 — The command surface

*Plan P3.1 · Spec §5.2 · Design §3.4, §7 · A-4, FR-23*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P3.1.a | One command per core capability, spelled in Design §7's vocabulary: `create`, `add`, `list`, `save-copy`, `replace`, `delete`, `check`, `info`, `password` | A-4, Design §7 | T3.1, T3.29 |
| P3.1.b | Files addressed by their stored path — folder metadata and name together — never by an internal identifier | FR-13, Design §7 | T3.1 |
| P3.1.c | A path matching more than one stored file refused, naming how many it matched, rather than acting on an arbitrary one | FR-13, HC-4 | T3.4 |
| P3.1.d | A path matching nothing reported as naming nothing, distinct from damage and from a wrong password | FR-2, HC-3, Spec §6 | T3.5 |
| P3.1.d2 | `add` refusing a path the vault already holds, naming it and pointing at `replace` | FR-34, Spec §6 | T3.4 |
| P3.1.e | `add` accepting both a file and a folder, reporting every path the walk declined | FR-9, FR-10, FR-11 | T3.1, T3.31 |
| P3.1.f | `list` filtering by folder and by name substring, so FR-7's grouping has a command-line equivalent | FR-6, FR-7 | T3.6 |
| P3.1.g | No flag anywhere that schedules, times, or conditions an operation on a threshold | FR-23, Spec §5.2 | T3.3 |
| P3.1.h | The four FR-29 statements attached to the moments that produce them: unrecoverability at `create`, the retained original after `add`, the persistence of deleted bytes at `delete`, the unprotected copy at `save-copy` | FR-29, HC-7, Design §7 | T3.26, T3.27, T3.33 |

**Why P3.1.b.** The core addresses files by `EntryId`, which is an integer that changes on every replace. Exposing it would put a number in the user's shell history that means a different file tomorrow. The stored path is the identity FR-13 already fixed, so the CLI resolves a path to an identifier itself and the identifier never appears on screen.

**Why P3.1.d is an item rather than a consequence.** The core reported a path that matches nothing as `Corrupt` with an empty affected list. That is the FR-2 mistake in another suit — naming nothing and being damaged are different conditions with different remedies. It is fixed in the core rather than papered over in the CLI, so the GUI inherits the fix instead of repeating the workaround (*Notes for Upstream*, item 1).

---

## P3.1a — Checking for damage

*Plan P3.1a · Spec §4.8, §5.2 · FR-33, S-4*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P3.1a.a | `check` reading and authenticating every file, reporting each failure by name | FR-33, S-4 | T3.20 |
| P3.1a.b | A non-zero exit when any file fails, so a backup script can use it without parsing output | Spec §5.2, FR-33 | T3.20 |
| P3.1a.c | Failure reported per file and the check continuing, so one damaged pack yields the full list of what it cost | S-4 | T3.20 |
| P3.1a.d | A plain statement in the result that Veil2 cannot recover a damaged file | Design §4.2, S-4 | T3.20 |

`check` is the one operation the Spec explicitly permits a scheduled script to run: it only reads. That permission is why P3.1a.b is worth its own item — an exit code is the whole interface when nobody is watching.

---

## P3.2 — Human output

*Plan P3.2 · Design §3.4, §7*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P3.2.a | A table in the GUI's column order, one row per file | Design §3.4 | T3.6 |
| P3.2.b | Sizes human-readable, counts exact and never rounded | Design §7 | T3.6, T3.32 |
| P3.2.c | Stored names printed exactly as stored, whatever the script | HC-8 | T3.7 |
| P3.2.d | Column widths computed on character count, with the misalignment this causes for double-width scripts stated rather than hidden | HC-8, Design §7 | T3.7 |
| P3.2.e | `info` printing the FR-8 figures including reclaimable space as a share of physical | FR-8, FR-22 | T3.32 |

**P3.2.d is an honesty clause, not a feature.** Aligning a column that contains Han or Hangul needs each character's display width, which needs a Unicode width table this project does not carry. The name is always correct; the column edge sometimes is not. Padding the name to make the column straight would be the one failure HC-8 exists to prevent, so the column loses.

---

## P3.3 — Machine output

*Plan P3.3 · Design §3.4*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P3.3.a | A `--format json` mode carrying the same facts as the table, with exact byte counts rather than human-readable sizes | Design §3.4 | T3.8 |
| P3.3.b | The human default never machine-shaped — no mode where one output serves both | Design §3.4 | T3.9 |
| P3.3.c | Machine output on standard output alone, valid from the first byte to the last with nothing else interleaved | Design §3.4 | T3.9, T3.16 |
| P3.3.d | Failures in machine mode reported in that mode, so a script parsing output does not meet prose only when something goes wrong | Design §3.4, §4.2 | T3.8 |

---

## P3.4 — Password input

*Plan P3.4 · Spec §5.2 · Design §3.4 · HC-2*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P3.4.a | Password read from a file named by `--password-file`, or from an environment variable | Spec §5.2 | T3.10, T3.11 |
| P3.4.b | **No option anywhere that takes a password as an argument value** — visible in process listings and shell history | HC-2, Spec §5.2 | T3.2, T3.13 |
| P3.4.c | An interactive prompt with the input not echoed, used only when a terminal is attached | HC-2, Design §3.4 | T3.10 |
| P3.4.d | A non-interactive invocation with no password supplied failing immediately, naming the missing input, never blocking on a prompt | Design §3.4, Spec §5.2 | T3.12 |
| P3.4.e | One trailing newline trimmed from a password file, and nothing else — a file written by `echo` is the common case, and trimming more would silently change the password | Spec §5.2 | T3.15 |
| P3.4.f | The second password of `create` and `password` supplied the same way, confirmed by re-entry when prompted interactively | FR-1, FR-4, C-4 | T3.33 |
| P3.4.g | A wrong password reported as a wrong password, with its own exit code, distinct from a damaged vault | FR-2 | T3.14 |

---

## P3.5 — Progress and cancellation

*Plan P3.5 · Design §3.4 · A-3, FR-14, FR-19*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P3.5.a | Progress written to standard error, results to standard output | Design §3.4 | T3.16 |
| P3.5.b | An updating single line when standard error is a terminal | Design §3.4 | T3.17 |
| P3.5.c | Periodic lines with no control characters when it is not, no more often than a fixed interval — initially every 2 seconds; tune with use | Design §3.4 | T3.17 |
| P3.5.d | An interrupt signal driving the core's cancel rather than killing the process, so a cancelled add leaves the vault as though it had not started | FR-14, FR-19, HC-4 | T3.18 |
| P3.5.e | A cancelled operation exiting with the cancelled code and stating what it left behind | FR-14, FR-19, Spec §6 | T3.18 |

**Why P3.5.d matters more than it looks.** HC-4 already requires that killing the process mid-add be safe, and Phase 4 will prove it. But FR-14 asks for something stronger — that a cancelled ingest leave the vault *as though it had not been started* — and the core delivers that only when told to stop cooperatively. Without a signal handler the CLI cannot reach the mechanism Phase 2 built, and the strongest guarantee in the product would be unreachable from the only frontend that exists.

---

## P3.6 — Exit codes

*Plan P3.6 · Spec §5.2, §6 · FR-2*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P3.6.a | One exit code per Spec §6 error class, so a script tells a wrong password from a damaged vault without parsing text | Spec §5.2, §6, FR-2 | T3.19 |
| P3.6.b | The mapping in one place, exhaustive over the error enum, so a new variant cannot silently become "general failure" | Spec §6 | T3.19 |
| P3.6.c | The codes documented in `--help`, since an interface a script depends on that is only discoverable by experiment is not an interface | Design §3.4 | T3.19 |

The mapping now lives in **Spec §5.2**, where a compatibility obligation belongs. This phase implements that table and adds nothing to it.

---

## P3.7 — The test suite

*Plan P3.7 · Spec §9 · A-4*

| Item | Work | Cites | Tests |
|---|---|---|---|
| P3.7.a | `assert_cmd` driving the built binary as a subprocess, over the full command surface | Spec §9, A-4 | all |
| P3.7.b | Every case asserting the exit code, not merely failure | Spec §5.2 | T3.19 |
| P3.7.c | Non-interactive cases run with no terminal attached, which is the condition they exist to check | Design §3.4 | T3.12, T3.17 |
| P3.7.d | An audit over every command's help text and every message for the Design §7 forbidden words | Design §7 | T3.29 |
| P3.7.e | An audit that no output contains a password, key material, or file content | HC-2, Spec §6 | T3.30 |

**Phase 2's rule is deliberately broken here, once.** Its cases required no subprocess, because needing one would have been evidence against the API's independence from a frontend. Phase 3's subject *is* a process — its exit code, its two streams, its behaviour with no terminal — so driving it any other way would test something other than the product.

---

## Notes for Upstream

Recorded per G-24. **All six were absorbed by the owner before this list was approved**, so the pins in the header are the versions that already contain them.

1. **Spec §6 had no variant for "no such file".** The core reported a path that matches nothing as `Corrupt` with an empty affected list — the conflation FR-2 exists to prevent, one level down. *Absorbed: `NotFound` in Spec §6 (v1.3). Phase 3 also uses the core's own refusal rather than pre-checking, so the GUI inherits the fix.*

2. **The exit-code numbers belong in Spec §5.2.** Once a backup script tests `$? -eq 5`, the number carries a compatibility obligation. *Absorbed: the table is Spec §5.2 (v1.3).*

3. **Design fixed no column order.** §3.4 required the CLI table to match the GUI's columns; §3.2 never enumerated them. *Absorbed: **name, folder, size, added** in Design §3.2 (v1.2).*

4. **Four dependencies ship for the first time in this phase.** *Absorbed into the Spec §7 table (v1.3): `anyhow` (binaries only; §6's prohibition on it inside the library stands), `serde_json`, `ctrlc`, `rpassword`.*

5. **Whether `add` may store two files at the same path.** *Absorbed: **FR-34** (Requirements v1.2) refuses it, with `AlreadyExists` in Spec §6. P3.1.c stands regardless — vaults written before this exist, and a duplicate already in one still has to be reported rather than guessed at.*

6. **`--force` for overwrite at the destination.** FR-18 requires confirmation naming the file; a non-interactive invocation cannot be asked. *Absorbed as a reading rather than a document change: this phase refuses without a flag and treats `--force` as the pre-confirmation. Recorded here because it is a reading of FR-18, not something FR-18 says.*

---

## Open Questions

*None outstanding.* Both were resolved by the owner at approval:

### Resolved before v1.0

- **~~Whether `list` defaults to grouping by folder or to a flat list.~~** Resolved: **flat**, with `--group` to group. A flat table pipes; a grouped one does not.
- **~~Whether a vault path may be defaulted from an environment variable.~~** Resolved: **no** — the vault is always an argument. One more place a path can come from silently is one more way to write to the wrong vault.
