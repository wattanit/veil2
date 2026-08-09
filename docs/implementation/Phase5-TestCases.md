# Veil2 — Phase 5 Test Cases: Portability by Construction

**Version:** 1.0
**Status:** draft
**Date:** 2026-08-09
**Owner:** wattanit
**Foundation and plan versions these cases are built against (G-14):**
- Requirements Document **v2.0** — upstream
- Design Guideline **v2.0** — upstream
- Technical Specification **v2.0** — upstream
- Implementation Plan **v2.0** — upstream
- [Phase5-ToDo.md](Phase5-ToDo.md) **v1.0** — companion; each case names the item it covers

This document owns the **enumerated checks that close Phase 5**. Every case cites the requirement it verifies (G-10).

---

## Conventions

**Case identifiers** are `T<phase>.<n>` — section-numbering references, not foundation identifiers (G-19).

**Where these run.** The development machine, macOS. Every case is written against a rule stated for all three platforms (Spec §8.1); what runs here is evidence for this platform, and T5.11–T5.13's fixture is what P8.2 carries to the other two rather than re-deriving the same evidence there.

**How to run them.**

```bash
cargo test --workspace                                          # everything except what is marked ignored
cargo test -p veil-core --test scale -- --ignored --nocapture    # T5.18, T5.19
```

Nothing in this phase needs a killed process or a release build — that machinery belongs to Phase 4's suite and is untouched here. The scale cases are `#[ignore]` for the same reason Phase 4's are: they cost minutes and, for T5.18, disk.

---

## Name normalisation

### T5.1 — An NFD name and its NFC spelling store as one entry
*Covers P5.1.a, P5.1.b · Verifies Spec §4.6, HC-8*

Add a file whose name is supplied pre-composed (NFC) and, separately in a fresh vault, add a file whose name is supplied as its NFD decomposition — same visible characters, different bytes. Also drive the same pair through `add_folder` over a directory tree containing both spellings as sibling files.

**Verdict:** in both vaults, the entry's stored `name` is byte-identical NFC, and the two source spellings that went in through `add`/`add_path` land on the same normalised form. Through `add_folder`, the NFC and NFD siblings are two files on disk beforehand and remain two distinct entries afterward — normalisation changes the bytes an entry is stored under, not how many files a folder walk found — which is the case that would catch normalisation being applied at the wrong point (comparing pre-walk instead of storing post-walk).

### T5.2 — Matching a stored name by its other spelling
*Covers P5.1.a, P5.1.c · Verifies Spec §4.6, FR-13*

Store a file whose name is NFC. Then call `replace` and, separately, `find`, supplying the NFD spelling of the same name.

**Verdict:** both resolve to the entry that is already there — `replace` replaces it, `find` returns it — because the query string is normalised before the comparison Spec §4.6 fixes as "exact and case-sensitive after normalisation." A caller typing or pasting a name is not expected to know which spelling their input method produced.

### T5.3 — Case sensitivity is unaffected by normalisation
*Covers P5.1.c · Verifies Spec §4.6*

Store `Report.PDF` and, in the same folder, add `report.pdf`.

**Verdict:** both succeed as two distinct entries. Normalisation changes NFC/NFD equivalence, nothing about case; §4.6 fixes comparison as exact and case-sensitive, and this case is where a comparison that accidentally folded case while it was being touched for normalisation would be caught.

### T5.4 — A folder walk over NFD-yielding paths produces NFC folder metadata
*Covers P5.1.b · Verifies Spec §4.6, FR-10*

Build a directory tree with a folder name and a file name that are only representable as their NFD forms in the constructed path (as macOS's filesystem APIs would hand back), and run `add_folder` over it.

**Verdict:** the entry's `folder` and `name` are both NFC. This is the case Spec §4.6's own rationale names directly — "macOS presents NFD from its filesystem APIs" — and it is the one where P5.1.b's second normalisation site (inside the walk, not only inside `add`) actually matters, because folder metadata built by string-joining path segments (`relative_folder`) never passes through a single choke point otherwise.

---

## Extraction representability

### T5.5 — A reserved device name is refused, not silently altered
*Covers P5.2.a, P5.2.b, P5.2.c · Verifies FR-31, HC-8*

Store an entry named `CON.txt`. Extract it with a destination folder as the target (the `to.is_dir()` path).

**Verdict:** the operation returns `Error::NameNotRepresentable` naming the entry and a reserved-name reason; nothing is written to the destination folder; no file appears under a substituted or truncated name. HC-8 makes the vault's own name authoritative, and a file that showed up under a different name than the vault reports would be exactly the confusion FR-31 exists to prevent.

### T5.6 — Every name in the reserved set is refused, and nothing outside it is
*Covers P5.2.b, P5.2.f · Verifies FR-31*

Store one entry for each of `CON`, `PRN`, `AUX`, `NUL`, `COM1`, `COM9`, `LPT1`, `LPT9`, a name containing `:` and one containing `*`, a name ending in a trailing space, and a name ending in a trailing dot. Alongside them, store `CONSOLE.txt`, `CON-fig.txt`, and a name containing a colon-free punctuation mark that is legal everywhere.

**Verdict:** every name in the first group is refused with a reason matching what makes it illegal; every name in the second group extracts normally. `CONSOLE.txt` and `CON-fig.txt` exist specifically to catch a check implemented as "starts with a reserved prefix" rather than "matches a reserved name exactly" — Windows reserves `CON`, not everything that begins with those three letters.

### T5.7 — A case collision is refused only where it matters
*Covers P5.2.b, P5.2.e, P5.2.f · Verifies FR-31, HC-8*

Store `Photo.jpg` and `photo.jpg` as two entries in the same folder. Extract each into a fresh destination folder.

**Verdict:** the check that runs for a case-insensitive destination (P5.2.f) refuses whichever of the two would extract second, naming the collision; a check running for a case-sensitive destination allows both. The two entries are both legitimate, distinct contents of the vault — this is the case Spec §4.6 exists to prevent producing an ambiguous or overwritten result from, not a case the vault should have refused to store in the first place.

### T5.8 — The collision check looks at the vault, not at what is already on disk
*Covers P5.2.e · Verifies Spec §4.6, HC-8*

Store `Photo.jpg` and `photo.jpg` in one folder, with the destination folder for extraction empty beforehand.

**Verdict:** the refusal in T5.7 still fires with nothing on disk yet to collide with. A check that only inspected the destination directory's current contents would miss this entirely on a first extraction and only catch it on a second — which is the wrong half of the problem, since the collision is a fact about the vault's two entries, not about extraction order.

### T5.9 — `save_copy` surfaces the refusal without touching the destination
*Covers P5.2.d · Verifies FR-31, Spec §5.2*

Run `veil save-copy` for `CON.txt` from T5.5's vault, with `--to` a destination folder.

**Verdict:** the command exits with a distinct, documented code; the destination folder is unchanged — no partial file, no folder created if it did not already exist; stderr names the entry and the reason. Design §6's interactive "ask for an alternative" is Phase 7's screen; this command has no window to ask in, so it refuses cleanly instead, which is what a script depending on this exit code needs.

### T5.10 — An extraction to an exact, caller-chosen path is unaffected
*Covers P5.2.d · Verifies FR-31*

Run `veil save-copy` for the same `CON.txt` entry with `--to` an exact file path rather than a folder.

**Verdict:** the file is written under the name the caller gave it. P5.2's check exists for the moment the vault's own name becomes the destination filename; when the caller names every character themselves, there is nothing for FR-31 to protect against, and refusing here would be stopping a write that could not possibly reproduce the confusion the requirement is about.

---

## The portability fixture

### T5.11 — The fixture opens and its manifest matches what it holds
*Covers P5.3.a, P5.3.d · Verifies Spec §9, HC-8*

Open the committed fixture at `crates/veil-core/tests/fixtures/portability/` and compare its entry list — folders, names, sizes — against the recorded manifest.

**Verdict:** exact match, including every script the manifest claims (Latin, Thai, Arabic, Han, emoji), the NFC/NFD pair, and every reserved name P5.2.b enumerates. This is the tripwire that catches the fixture and its manifest drifting apart before either T5.12 or P8.2 runs against a false premise.

### T5.12 — Every fixture entry extracts byte-identically
*Covers P5.3.b, P5.3.c · Verifies Spec §9, HC-8*

Extract every entry in the fixture and compare each against the manifest's recorded content.

**Verdict:** byte-for-byte match for all of them, and the NFC/NFD pair resolves to exactly one entry in the fixture's list, not two (P5.3.c) — proof, not argument, that Spec §4.6's normalisation claim holds for the exact pair the specification's own rationale names.

### T5.13 — The manifest states which reserved names refuse here
*Covers P5.3.d · Verifies FR-31, Spec §4.6*

Extract each of the fixture's reserved-name entries into a destination folder and compare the outcome against the manifest's per-entry expectation.

**Verdict:** each matches its recorded expectation — refused with the reason the manifest states, on this platform, with the case-sensitive destination this development machine's filesystem provides. The manifest also records what each entry's outcome would be on a case-insensitive or Windows destination, unverified here and unverifiable without one; P8.2 is where that half is checked.

---

## Network-path advisory

### T5.14 — A vault on ordinary local storage reports no network advisory
*Covers P5.4.a, P5.4.e · Verifies Spec §2*

Open a vault in a plain temporary directory on the local disk and read the advisory fact.

**Verdict:** false, and opening the vault does not spawn a second `mount` read or stat beyond what was already needed — checked by counting the process's own filesystem calls stays flat whether the fact is read or not, so S-2 is not put at risk by a feature nobody asked S-2 to pay for.

### T5.15 — A vault under a path `mount` reports as `nfs` or `smbfs` is flagged
*Covers P5.4.a, P5.4.b · Verifies Spec §2, FR-26*

Fabricate `mount`'s output (a fixed string given to the parser directly, rather than requiring an actual network mount on the test machine) naming a path as `nfs` or `smbfs`, and check a vault directory under that path.

**Verdict:** the advisory fact is true. Driving the parser directly rather than mounting a real network share is what makes this case runnable without networked storage or elevated permissions — the parsing logic is what P5.4 adds; `mount` reporting the type correctly is the operating system's job, not this suite's to re-prove.

### T5.16 — Windows' UNC-prefix check and its stated gap
*Covers P5.4.c · Verifies Spec §8.1, HC-8*

On the code path compiled for Windows (exercised here via the platform-independent unit under test, not a Windows machine — Phase 8 confirms it there), check a `\\server\share\vault` path and, separately, a path under a drive letter that a comment in the fixture marks as "mapped to a network share for this test's purposes."

**Verdict:** the UNC path is flagged; the mapped-drive path is not, matching the ToDo's stated limitation exactly rather than by accident. A case here that could not be made to fail — because the implementation happened to also catch mapped drives — would be evidence the note describing it as incomplete is out of date, which is worth knowing before Phase 8 arrives to find out the hard way.

### T5.17 — The advisory note appears once, in the right register
*Covers P5.4.d · Verifies Design §4.3, §7, Spec §2*

Open a vault flagged as on network storage through `veil open` (or any command that opens one) and read stderr.

**Verdict:** a note appears alongside the existing read-only note's position, worded as an honesty clause rather than a warning icon or an error, and it appears exactly once per command invocation — not once per internal open call, and not repeated by a second command run against the same vault unless that command opens it again. Design §7's fixed vocabulary and §4.3's register apply to this note exactly as they apply to the read-only one it sits beside.

---

## Scale

### T5.18 — Peak memory does not grow with a multi-gigabyte file
*Covers P5.5.a · Verifies S-1, C-2* — `#[ignore]`, run on request

Add and then extract a file several gigabytes in size, sampling the process's own peak resident memory throughout both operations.

**Verdict:** peak memory stays within a small constant multiple of the chunk size Spec §3.3 streams through, never approaching the file's size. An implementation that buffered a whole file before encrypting or decrypting it would pass every functional test in this suite and fail only here, which is why S-1 gets a case that actually measures rather than one that infers from the code streaming correctly elsewhere.

### T5.19 — Open time does not grow with entry count, at C-1's limit
*Covers P5.5.b · Verifies S-2, C-1* — `#[ignore]`, run on request

Build a vault holding 65,536 entries (C-1's maximum) and a second vault holding a handful, and compare the time each takes to open.

**Verdict:** the large vault's open time is close to the small vault's — both dominated by the same fixed costs (header read, one index-slot decrypt) — and nowhere near proportional to 65,536. Spec §4.3's whole-index-in-memory design and P4.4.a's "nothing at open" rule are both aimed at this property; this is the case that measures it at the size C-1 actually permits rather than at a size too small for a linear-cost implementation to show a difference.

---

## Path-metadata length

### T5.20 — A path over the limit is refused, naming both numbers
*Covers P5.6.a, P5.6.b, P5.6.c · Verifies FR-15, Spec §6*

Attempt to add a file whose `folder` and `name` together exceed the configured maximum by one byte.

**Verdict:** `Error::LimitExceeded` with `Limit::PathMetadata`, the configured maximum, and the length the attempt would have produced — matching C-1 and C-2's existing pattern exactly (P2.9's FR-15 case, re-run here against the limit this phase adds). Nothing is written; the vault after the attempt is indistinguishable from before it.

### T5.21 — A path at exactly the limit succeeds
*Covers P5.6.a · Verifies FR-15*

Add a file whose `folder` and `name` together equal the configured maximum exactly.

**Verdict:** it succeeds and extracts normally. The boundary is inclusive — this case is what stops "over the limit" in T5.20 from silently drifting into "at or over the limit" the next time either number changes.

---

## Not covered, and why

**A live network mount, exercised for real.** T5.15 and T5.16 drive P5.4's parsers directly with fabricated input rather than mounting NFS, SMB, or a UNC share on the test machine. Setting one up needs infrastructure this project has ruled out for the same reason Phase 4's crash tests avoid an indirection layer inside `veil-core`: it would put test-only setup cost into a suite meant to run on request with nothing but `cargo test`. What is proved is that the parser reads `mount`'s and `/proc/mounts`' documented output correctly; that a given machine's `mount` actually reports what this suite assumes it reports is environmental, not a defect in this code, and is exactly the category of gap Spec §2's own honesty clause already admits.

**Windows' `GetDriveType` for a mapped network drive.** Closing the gap T5.16 states would need a Win32 call this crate's `#![forbid(unsafe_code)]` does not allow without either an `unsafe`-internal dependency or a subprocess parse of `net use`'s output, and Phase 8 — the phase with an actual Windows machine to test it against — is a more honest place to make that call than guessing at it here.
