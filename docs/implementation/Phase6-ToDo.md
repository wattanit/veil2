# Veil2 — Phase 6 To-Do: GUI v1

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-10
**Owner:** wattanit
**Foundation and plan versions this list is built against:**
- Requirements Document v1.0 — upstream
- Design Guideline v1.2 — upstream (v1.1 added §8.7, §8.8; v1.2 corrected §8.7 after live testing showed its first interaction did not work — see P6.16)
- Technical Specification v1.0 — upstream
- Implementation Plan v1.1 — upstream; this list expands Plan tasks P6.1–P6.13, P6.15–P6.17

This document owns the step-level breakdown of Phase 6. It defers what to build to the Requirements, how it presents to the Design Guideline, how it is built to the Specification, and phase sequencing to the Implementation Plan. Enumerated checks live in [Phase6-TestCases.md](Phase6-TestCases.md).

**P6.14 (signing and notarisation) is out of scope for this document and this effort.** It needs a real Apple Developer ID certificate and notarisation credentials the owner holds; picked up separately once those are in hand. Everything else in Phase 6 — the whole GUI surface, and the platform statement of P6.15 — is in scope and is built to ship as the 2.0.0 release in one piece, not incrementally behind pre-release tags (Plan, Open Questions, resolved at Phase 5 exit).

---

## Conventions

**Item identifiers** are `P<phase>.<task>.<letter>`.

**Status** follows the Plan's convention: **built, carries forward** / **built, needs rewrite** / **built, needs review** / **not yet built**.

**Done** for an item means the cited behaviour is observable, the test cases listed against it pass, and the Plan's definition of done holds.

**Quoted strings are load-bearing.** Where the Design Guideline gives exact wording, that wording is what ships — not a paraphrase. Where it does not, the closest existing wording is the CLI's own (`veil-cli/src/report.rs`, `failure.rs`, `error.rs`), so the vocabulary audit (P6.13) starts from agreement rather than reconciling two inventions after the fact.

---

## What Phase 6 is for

Phase 5 proved the shell renders a vault's own filenames correctly. Phase 6 builds the product around it: every functional requirement reachable from the GUI, in the vocabulary and interaction policy the Design Guideline fixes, ending in the actual 2.0.0 release. *Proves the product* (Plan).

**What Phase 5 left standing that Phase 6 removes.** `open_fixture_vault` (debug-only) was the only way into a vault; there was no unlock screen, no create flow, no identity bar, no delete, no check-for-damage, and error handling was a flat string shown as-is rather than the three-part structure Design §4.2 requires. All of that is this phase's job.

**A vault's "name" is its directory's filename, nothing more.** Neither the header nor the index carries a name field (checked directly — `Header` and `IndexDocument` have no such field); Design's mockup ("Holiday Photos") and its Key Moments ("The user names the vault") both describe choosing where it lives, not a separate metadata field to design storage for. The GUI derives the displayed name from the chosen path's final component (stripping a `.veil` extension if present), the same as the CLI already does implicitly by taking a path.

**The backend command layer grows substantially.** Phase 5 built `open_vault`, `open_fixture_vault`, `list_entries`, `close_vault`, `cancel_operation`, `extract_entry`, `add_files`, `choose_save_path` — enough to prove the shell, not enough to reach every requirement. This phase adds `create_vault`, `choose_vault_path` (a native folder dialog, for both opening and creating — there is currently no way to pick a path at all outside the debug fixture), `delete_entry`, `replace_entry`, `choose_source_paths`, and `check_vault` (drives `Vault::verify`). It also restructures every command's error into a small structured shape — `{ kind, message }` — instead of a flat string, because P6.10's three-part presentation and P6.11's per-condition responses need to branch on *which* condition occurred, not parse English out of a string built for a human.

**`add_files` was rebuilt around identity, not position, after live testing found the position-based design did not work.** The first version of P6.16 (Design §8.7 v1.1) detected "dropped onto row X" from the webview's reported drag position; tested live, that position was wrong regardless of where the cursor actually was, reproducibly, and the standard DOM drag events that could have been a fallback never fire at all here (the webview's own native handler consumes the OS drag first). Rebuilt so `add_files` itself detects a collision — a dropped or chosen path whose folder *and* name together already match an entry — and reports it as a `Collision` for the frontend to confirm before replacing, rather than either interaction needing to know where anything was dropped. Design §8.7 is now v1.2 to match. `add_folder`'s own early-abort-on-first-collision behaviour (from `Vault::add_folder`, which returns on the first `Err`) is why `add_files` walks a dropped folder itself via `veil_core::vault::walk` and calls `Vault::add` per file, rather than calling `add_folder` as a black box — a single collision partway through a large folder must not cost every file after it its own add.

**A dropped folder's own name was, for a time, dropped from its files' stored paths — a correctness bug, not a cosmetic one.** `walk`'s contract is "relative to the root, empty at the root itself"; `add_folder` and `add_files` originally stored that relative path as-is, so a file directly inside one added folder (say `Reports/`) and a same-named file directly inside a *different* added folder (say `Archive/`) both stored as folder `""` — indistinguishable identities for two files that are not the same file, and a second add of either would look like a legitimate replace of the other. Confirmed live: dropping a folder with files nested under sub-folders correctly grouped by those inner folders, which is what initially looked fine, but a file directly at the dropped folder's own top level had nowhere of its own to land. Fixed in both `Vault::add_folder` (`veil-core`) and the GUI's `add_files` (which walks a dropped folder itself rather than calling `add_folder`, per the note above) by prepending the added root's own name as the top-level folder segment — `Reports/x.bin` and `Archive/x.bin` now store as folder `"Reports"` and `"Archive"` respectively, never colliding. Requirements FR-10 and Technical Specification §4.7 are corrected to state this plainly; the previous wording ("relative to the added root") was the ambiguity that produced the original implementation.

**Two CLI capabilities had no GUI interaction designed for them: `replace` and changing the password.** Neither appeared in the Design Guideline's Key Moments through v1.0, nor in Plan's original P6.1–P6.13. Rather than build ahead of an undesigned screen or silently ship without them, the Design Guideline gained §8.7 and §8.8 (v1.1) and the Plan gained P6.16 and P6.17 (v1.1) — the same sequencing every other capability in this rewrite went through, just done now instead of in an earlier pass.

**`Error::FormatSuperseded` and `Error::VerificationFailed` are currently unreachable by any real vault**, checked directly rather than assumed: `CURRENT_FORMAT_VERSION` and `OLDEST_SUPPORTED_FORMAT_VERSION` are both `1` — this is the first format version this codebase has ever written, so nothing is yet superseded — and nothing in `veil-core` constructs `VerificationFailed` (the CLI's own `check` command builds its own `Failure::Damage` from `Vault::verify`'s `Report` instead of that variant). P6.2's superseded-format handling and P6.12's damage-report handling are built and asserted against a deliberately-malformed header/entry, the same way Phase 1's corruption suite tests conditions no ordinary use produces, not against a vault this phase can create by ordinary means.

---

## P6.0 — Command layer and structured errors

*Supports every task below · A-4, Design §4.2*

Not a Plan task on its own — the shared foundation every other P6.x command needs, broken out so its own correctness is checked once rather than seven times.

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P6.0.a | Done | Every command's error becomes `{ kind: string, message: string }` instead of a flat `String`, `kind` one of the `Error` enum's variant names, `message` the existing `Display` text. The frontend branches on `kind`; `message` is what actually renders | Design §4.2 | T6.1 |
| P6.0.b | Done | `create_vault(path, password) -> VaultSummary` — calls `Vault::create` with `KdfParams::for_new_vaults()` (never `for_tests()` — that is compiled out of release for exactly this reason, P1.1.d) | FR-1, HC-5 | T6.2 |
| P6.0.c | Done | `choose_vault_path(mode: "open" \| "create") -> Option<String>` — a native folder dialog (`tauri_plugin_dialog`, called from Rust, as `choose_save_path` already is), so a path reaches `open_vault`/`create_vault` without the fixture bypass | Design §5, §8.1, §8.2 | T6.2, T6.3 |
| P6.0.d | Done | `VaultSummary` gains `access: "readOnly" \| "readWrite"` (from `Vault::access()`) and `unreadableCount: u64` (from `Vault::unreadable_entries().len()`), so the identity bar has both facts the moment a vault opens, per Design §4.3's "stated when the vault opens... rather than discovered when a write fails" | Design §3.2, §4.3, FR-23, S-3 | T6.2, T6.9 |
| P6.0.e | Done | `delete_entry(id: u64) -> Result<(), ErrorInfo>` — calls `Vault::delete` | FR-22 | T6.15 |
| P6.0.f | Done | `check_vault() -> CheckReport` — drives `Vault::verify`, progress over the existing `"operation-progress"` event, cancellable through the existing `cancel_operation`. `CheckReport { checked: u64, complete: bool, failures: Vec<{ id, name, folder, damage }> }` | FR-26, S-3, Design §8.6 | T6.22–T6.26 |

---

## P6.1 — Unlock screen (and first run)

*Plan P6.1 · Design §5, §8.1 · FR-2, C-3*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P6.1.a | Done | First run, no vault picked: exactly two choices, "Create a vault" and "Open a vault", nothing else — no tour, no sample vault | Design §8.1 | T6.2 |
| P6.1.b | Done | Unlock screen shows four things only: the vault's name (its path's final component), its location (the full path), a password field, an unlock button | Design §5 | T6.3 |
| P6.1.c | Done | The unlock button shows a visibly-alive working state during derivation (~1s, C-3) and the password field locks; no fake percentage, since there is nothing measurable to show | Design §5, C-3 | T6.4 |
| P6.1.d | Done | Wrong password and a damaged vault are distinct screens (FR-2): `WrongPassword` → *"That password didn't work. Try again."*, no attempt count, no lockout. `NotAVault`/`Corrupt` → *"This vault can't be read. It may be incomplete or damaged."* plus what is known, and advice to work from a backup | Design §5, FR-2 | T6.5, T6.6 |
| P6.1.e | Done | No "remember this password" anywhere (HC-7) | Design §5, HC-7 | T6.7 |

---

## P6.2 — Superseded and too-new format messages

*Plan P6.2 · Design §5 · FR-5, FR-6*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P6.2.a | Done | `FormatTooNew` names the version this release understands versus the version the vault needs | Design §5, FR-5 | T6.8 |
| P6.2.b | Done | `FormatSuperseded` states the vault's format version, that it opens normally, and that a future release may offer to convert it — built and asserted against a header with a deliberately out-of-range version, since no real vault this codebase writes can be superseded by itself yet | Design §5, FR-6 | T6.8 |

---

## P6.3 — Creating a vault

*Plan P6.3 · Design §8.2 · HC-7, C-4, FR-1, FR-27*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P6.3.a | Done | The user names the vault (by choosing where it lives, P6.0.c) and sets a password | Design §8.2, FR-1 | T6.9 |
| P6.3.b | Done | Before creation, a `caution`-styled block states the exact HC-7 disclosure: *"If you forget this password, everything in this vault is lost. There is no recovery, no reset, and no way for anyone — including us — to get it back. Write the password down and keep it somewhere safe."* Acknowledged by an explicit, un-checked checkbox — never pre-ticked | Design §8.2, HC-7, FR-27 | T6.10 |
| P6.3.c | Done | A password under C-4's 12-character minimum is refused with `PasswordTooShort`'s message before `create_vault` is even called client-side, and by the backend regardless (`veil-core` enforces this; the frontend duplicating the check is a faster failure, not the authority) | C-4 | T6.11 |
| P6.3.d | Done | Strength feedback, if shown, is descriptive only — may say a password is short, never that one is "strong" (§7's forbidden-claims list: nothing here should look like a claim about attacker resources) | Design §8.2, §7 | T6.11 |
| P6.3.e | Done | No "skip" option on the disclosure | Design §8.2 | T6.10 |

---

## P6.4 — Identity bar

*Plan P6.4 · Design §3.2 · FR-7*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P6.4.a | Done | Vault name, lock state, and a lock action, always visible while a vault is open — confirmed live | Design §3.2 | T6.12 |
| P6.4.b | Done | Satisfied by construction rather than by a dedicated indicator: locked and unlocked are entirely different screens (P6.9.d), which differ in every respect, not only colour — a stronger distinction than a dot that changed shade would give | Design §3.2 | T6.26 |
| P6.4.c | Done | Read-only state (P6.0.d) shown in the identity bar the moment the vault opens: *"Read-only — this vault can't be changed from here."* | Design §4.3, FR-23 | T6.9, T6.16 |
| P6.4.d | Done | Statistics line — entry count and total stored size — visible immediately at open, no loading state (open time does not scale with vault size, S-2) — confirmed live | Design §3.2, FR-7 | T6.13 |

---

## P6.5 — Search and folder grouping

*Plan P6.5 · Design §3.2 · FR-8*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P6.5.a | Done | Search filters the already-loaded list client-side (no new command — `list_entries` already returns everything); matches name and folder | Design §3.2, FR-8 | T6.14 |
| P6.5.b | Done | A grouping toggle collapses/expands by recorded folder path — a flat view control, not a tree: no create, rename, or drag, no nesting | Design §3.2, §1.2, FR-8 | T6.14 |

---

## P6.6 — Adding files

*Plan P6.6 · Design §8.3 · FR-9, FR-15, FR-27*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P6.6.a | Done | The drop target (built in Phase 5) and an explicit "Add files…" control both reach the same `add_files` command — confirmed live | Design §3.3, §8.3 | T6.20 |
| P6.6.b | Done | On completion, the exact FR-27 disclosure once: *"Added {N} files. The originals are still on your disk — Veil doesn't delete them."* | Design §8.3, FR-27 | T6.20 |
| P6.6.c | Built, carries forward | Progress and cancel reuse the existing operation-bar machinery from Phase 5's event channel; a cancelled add leaves the vault as though it had not started (FR-15) | Design §4.1, FR-15 | T6.21 |
| P6.6.d | Done | Offering to reveal the originals in the file manager is fine; no control to delete them (secure erasure is out of scope, Requirements §2.3) — no reveal control built either, since Design does not require one | Design §8.3 | T6.20 |
| P6.6.e | Done | A dropped path that is a folder is walked (FR-10) via `veil_core::vault::walk` called directly rather than `Vault::add_folder`, not handed to the single-file add path, which would try to read the directory itself as a file's content and fail — confirmed live this was happening before the fix | Design §3.3, FR-10 | T6.37 |
| P6.6.f | Done | The dropped folder's own name is preserved as the top-level folder segment, not discarded — confirmed live a flat drop looked fine while a folder's own top-level files silently collided with any other added folder's top-level files (see note above); fixed in `Vault::add_folder` and mirrored in `add_files` | FR-10, FR-14 | T6.20, T6.33, T6.37 |

---

## P6.7 — Extracting ("save a copy")

*Plan P6.7 · Design §6 · FR-17, FR-19, FR-27*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P6.7.a | Done | The destination is always chosen and shown — no default folder, no one-click extract (already true of Phase 5's `choose_save_path`; carried forward) | Design §6, FR-17 | T6.20 |
| P6.7.b | Done | An overwrite is confirmed by name: *"Replace {file} in {folder}?"* | Design §6, FR-19 | T6.21 |
| P6.7.c | Done | A damaged entry's extraction removes the partial output and states plainly that the file was damaged in the vault, the incomplete copy was removed, and the vault's other files are unaffected | Design §6, FR-18 | T6.16 |
| P6.7.d | Done | On success, the exact FR-27 disclosure once, as a completion-state line, not a warning dialog: *"Saved to {folder}. This copy is not protected."* | Design §6, FR-27 | T6.20 |
| P6.7.e | Done | The user-facing verb throughout is "save a copy", never "extract" or "decrypt" — closes the wording gap Phase 5's own status text left (`"Extracting {name}…"`) | Design §7 | T6.31 |

---

## P6.8 — Deleting

*Plan P6.8 · Design §8.4 · FR-22, FR-27*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P6.8.a | Done | Confirmed with `caution` styling, naming the file or the exact count: *"Delete {file}?"* / *"Delete {N} files?"* | Design §4.1, §8.4 | T6.15 |
| P6.8.b | Done | Deletion is immediate: the entry leaves the list and becomes unreachable at once (FR-22) | FR-22 | T6.15 |
| P6.8.c | Done | No claim about the underlying storage being freed — Spec §4.5's residue model means it may not be, immediately, and Design leaves the presentation of any separate reclaim step undesigned until one exists | Design §8.4 | T6.15 |

---

## P6.9 — Locking and the locked screen

*Plan P6.9 · Design §8.5 · FR-3, HC-1*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P6.9.a | Done | Lock is one click from the identity bar, always available while a vault is open | Design §3.2, §8.5, FR-3 | T6.27 |
| P6.9.b | Done | Quitting the application locks the vault; there is no "leave it open" setting | Design §8.5, FR-3 | T6.28 |
| P6.9.c | Done | No automatic lock on inactivity, sleep, or screen lock — an explicit non-feature, not an oversight | Design §8.5, FR-3 | T6.29 |
| P6.9.d | Done | The locked state is its own screen, not a greyed-out list — a visible file list while locked would suggest the index is still readable (HC-1) | Design §8.5, HC-1 | T6.30 |

---

## P6.10 — Three-part error presentation

*Plan P6.10 · Design §4.2*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P6.10.a | Done | Every error surface (P6.0.a's structured `{ kind, message }`) renders three parts in order: what happened, the current state, available actions — the current-state part is the one most often skipped and the one Design calls out as most important | Design §4.2 | T6.16 |
| P6.10.b | Done | Errors render at the location of the action (the extraction, the add, the delete) — never as a system notification | Design §4.2 | T6.16 |

---

## P6.11 — Constrained conditions

*Plan P6.11 · Design §4.3 · FR-16, FR-23, FR-24, FR-25, S-3*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P6.11.a | Done | `VaultInUse`: state it is open elsewhere, offer to retry. No read-only fallback offered — FR-23 does not provide one around genuine lock contention — and no offer to break the lock | Design §4.3, FR-23 | T6.9 |
| P6.11.b | Done | Read-only at open (P6.0.d, P6.4.c): controls that would write are disabled, not hidden, with the reason available on the control | Design §4.3, FR-23 | T6.9, T6.16 |
| P6.11.c | Done | `ChangedOnDisk`: stop before writing, state something external changed the vault, offer to reload — never merge or overwrite silently | Design §4.3, FR-24 | T6.16 |
| P6.11.d | Done | `StorageUnavailable`: name the volume, state the vault is intact as of the last completed step, return to a usable state without a restart | Design §4.3, FR-25 | T6.16 |
| P6.11.e | Done | Destination full during extraction: the partial file is removed (already true — `extract_to_path` removes it on any failure); state how much space was needed against how much was available | Design §4.3 | T6.16 |
| P6.11.f | Done | `LimitExceeded`: name the limit and the current value in the same message | Design §4.3, FR-16 | T6.16 |
| P6.11.g | Done | A damaged entry marks itself in the list rather than failing the whole vault — the rest stays usable and looks it (S-3) | Design §4.3, S-3 | T6.9, T6.16 |

---

## P6.12 — Checking a vault for damage

*Plan P6.12 · Design §8.6 · FR-26, S-3*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P6.12.a | Done | User-initiated only, never scheduled; states a time estimate (not a byte count) before starting, since the decision is whether to wait | Design §8.6, FR-26 | T6.22 |
| P6.12.b | Done | Progress per entry; cancellable throughout; a cancelled check reports what it completed rather than discarding the result | Design §8.6 | T6.23 |
| P6.12.c | Done | A clean result is stated narrowly: *"Checked {N} files. No damage found."* — never "your vault is healthy" | Design §8.6 | T6.24 |
| P6.12.d | Done | A failed result names the files and states plainly that Veil cannot recover them: *"{N} files are damaged. Their data in this vault can't be recovered — Veil doesn't keep a spare copy. If you have a backup, restore these files from it."* followed by the file list | Design §8.6, S-3 | T6.25 |
| P6.12.e | Done | Damaged entries stay marked in the list after the dialog closes, using P6.11.g's treatment — the result remains visible, not only in a dismissed dialog | Design §8.6, S-3 | T6.26 |

---

## P6.13 — Vocabulary audit

*Plan P6.13 · Design §7*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P6.13.a | Done | Every GUI string checked against Design §7's fixed-vocabulary table (vault, file, folder, password, add, save a copy, lock, check for damage) and forbidden-word list | Design §7 | T6.31 |
| P6.13.b | Done | The same audit run over the CLI's existing strings — Design §7 fixes vocabulary "GUI and CLI alike"; a mismatch found in the CLI is this phase's to fix too, not a Phase 3 regression to reopen separately | Design §7 | T6.31 |
| P6.13.c | Done | No forbidden security-theatre language anywhere ("military-grade", "bank-level", "100% secure", "your data is safe", and the rest of §1.2's and §7's lists) | Design §1.2, §7 | T6.31 |

---

## P6.15 — Release platform statement

*Plan P6.15 · Requirements §2.1, §8*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P6.15.a | Done | The release states the platform it was run on — 2.0.0 is macOS; nothing claims Windows or Linux support, "coming soon" or otherwise | Requirements §2.1, §8 | T6.32 |

---

## P6.16 — Replacing a file

*Plan P6.16 · Design §8.7 (v1.2) · FR-13, FR-14, §4.1*

**Two interactions, per Design §8.7's v1.2 rewrite** — the first version of this item (v1.1) specified detecting a drop "onto its row" from the webview's reported drag position, built, and confirmed live not to work: the position was wrong regardless of where the cursor actually was, and the DOM's own drag events that could have been a fallback never fired at all. Rebuilt around identity instead of position.

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P6.16.a | Done | `replace_entry(folder, name, source_path) -> Result<EntryInfo, ErrorInfo>` — calls `Vault::replace` | FR-13 | T6.34 |
| P6.16.b | Done | **By identity, automatically:** `add_files` (P6.0) detects a dropped or chosen path whose folder and name already match an entry, and reports it as a `Collision` instead of adding or failing it. Matched on folder *and* name together — `Reports/x` never collides with `Archive/x` | Design §8.7, FR-14 | T6.33, T6.37 |
| P6.16.c | Done | **By explicit choice, for anything else:** a "Replace…" action on the selected entry, enabled only while one is selected, opens a native file chooser with no constraint on the chosen file's own name | Design §8.7 | T6.34 |
| P6.16.d | Done | Confirmed by name (single) or count (several, naming each), stating the content loss plainly: *"Replace {file}? Its current content in this vault will be gone."* / *"Replace {N} files already in this vault? Their current content will be gone."* | Design §8.7, §4.1 | T6.34 |
| P6.16.e | Done | Name and folder stay the same; the list's "added" figure updates to the replacement's time, since that is the content now | Design §8.7 | T6.34 |

**A selected row had no visual style at all** — confirmed live: `.selected` was toggled correctly in script but nothing in `styles.css` rendered it, so a row chosen for Replace or Delete looked identical to one merely under the mouse. Fixed with its own state (a stronger `accent` fill plus an inset left stripe, distinct from hover's lighter tint even when the two coincide) rather than reusing hover's styling for both.

---

## P6.17 — Changing the password

*Plan P6.17 · Design §8.8 · HC-7, C-4*

| Item | Status | Work | Cites | Tests |
|---|---|---|---|---|
| P6.17.a | Done | `change_password(current, new) -> Result<(), ErrorInfo>` — calls `Vault::change_password` with `KdfParams::for_new_vaults()`, mirroring the CLI's own `password` command | HC-5 | T6.35 |
| P6.17.b | Done | Reached from the identity bar, alongside Lock — a labelled control, not an icon (Design §2.4) — confirmed live | Design §8.8, §2.4 | T6.35 |
| P6.17.c | Done | Requires the current password even though the vault is already unlocked — `veil-core`'s API requires it, and the UI does not work around that | Design §8.8 | T6.36 |
| P6.17.d | Done | The new password is typed twice and subject to C-4, same as creation; a mismatch or a too-short password is refused before the backend is called | C-4, Design §8.8 | T6.36 |
| P6.17.e | Done | A smaller HC-7 disclosure than creation's, exact wording: *"If you forget the new password, this vault is lost the same way it would be with the old one — there is still no recovery."* No strength claim | Design §8.8, HC-7, §7 | T6.36 |

---

## Exit

- Every functional requirement is reachable from the GUI, `replace` and password change included as of Design Guideline v1.2.
- The vocabulary audit is clean in both applications.
- P6.14 (signing, notarisation) is explicitly excluded from this exit — picked up separately.

**Met.** Every item above except P6.14 is confirmed live against the running application — the unlock, first-run, creation, identity bar, search and grouping, add/extract/replace/delete, lock, damage-check, and password-change screens were each exercised by hand, not just built and assumed. The automated suite covers what does not need a real webview to check (command shapes, error mapping, folder-walk and collision identity, progress mechanics, the vocabulary audit, the platform statement); everything else — screen composition, exact wording, visual state — was checked by hand against the Design Guideline the same way Phase 5 established. Three real bugs surfaced only by this live testing and are fixed and regression-tested: the drag-drop position data Tauri reports was unreliable (§8.7 rebuilt around identity, v1.2), a selected row had no visual style at all, and a dropped folder's own name was being dropped from its files' stored paths — a genuine identity-collision correctness bug, not a cosmetic one (see the note in "What Phase 6 is for").

---

## Open Questions

- **Whether the damage check of §8.6 is reachable from the identity bar or only from a menu** (carried from Design Guideline §9, unresolved there). Resolver: Design Guideline, next version — this phase places it somewhere reasonable and treats the exact location as tunable, not fixed.
- **Whether search covers folder metadata as well as filenames, and whether it is literal or fuzzy** (carried from Design Guideline §9). Resolver: Design Guideline, next version. This phase implements literal matching over both, as the smallest thing consistent with either future answer.
