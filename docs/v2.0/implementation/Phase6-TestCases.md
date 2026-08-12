# Veil2 — Phase 6 Test Cases: GUI v1

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-10
**Owner:** wattanit
**Foundation and plan versions these cases are built against:**
- Requirements Document v1.0 — upstream
- Design Guideline v1.2 — upstream
- Technical Specification v1.0 — upstream
- Implementation Plan v1.1 — upstream
- [Phase6-ToDo.md](Phase6-ToDo.md) v1.0 — companion; each case names the item it covers

This document owns the enumerated checks that close Phase 6. Every case cites the requirement it verifies. P6.14 (signing, notarisation) has no cases here — out of scope for this document, per Phase6-ToDo.md.

---

## Conventions

**Case identifiers** are `T<phase>.<n>`, sequential within this document.

**Three kinds of case**, following Phase 5's own distinction, now with a third added:
1. **Command-layer logic** — driven through `tauri::test`'s mock runtime, the same way Phase 5's T5.2/T5.3 drive it, no real webview needed.
2. **Rendering and interaction** — layout, wording, dialogs, drag-and-drop — checked live in `cargo tauri dev`, by a person looking at it, per Phase 5's own conventions section.
3. **Deliberately-unreachable conditions** — `FormatSuperseded`, and any damage case — built against a fixture deliberately corrupted or version-stamped for the purpose, the same way Phase 1's corruption suite works, because no vault this codebase writes today produces these conditions on its own.

**Where these run.** The development machine, macOS. Windows and Linux are not in scope (Requirements §2.1) and are not run.

---

## Command layer

### T6.1 — Every command's error carries a kind the frontend can branch on
*Covers P6.0.a · Verifies Design §4.2*

Trigger each of the `Error` variants reachable through the command layer (wrong password, corrupt, format-too-new, vault-in-use, read-only, changed-on-disk, storage-unavailable, limit-exceeded, cancelled, not-found, already-exists) through `tauri::test`'s mock runtime and inspect the returned error shape.
**Verdict:** every one carries a `kind` matching the `Error` variant's name and a `message` matching its `Display` text. A variant added later without a `kind` mapping fails this case.

### T6.2 — Creating a vault through the command layer matches the library directly
*Covers P6.0.b, P6.0.c, P6.1.a · Verifies FR-1, HC-5*

Call `choose_vault_path`'s underlying logic is exempted from automation (a native dialog); call `create_vault` with a path and a password meeting C-4, through the mock runtime.
**Verdict:** returns a `VaultSummary` with `entry_count: 0`; the vault opens afterward with `Vault::open` using the same password; the header's KDF parameters are `KdfParams::for_new_vaults()`'s, not `for_tests()`'s — confirmed by checking the recorded cost is the real one, not the cheap one.

### T6.3 — `VaultSummary` carries what the unlock screen and identity bar need
*Covers P6.0.d, P6.1.b · Verifies Design §3.2, §4.3, FR-23, S-3*

Open a read-write vault, a read-only vault (Phase 4's T4.12 technique), and a vault with one missing entry file, each through `open_vault`.
**Verdict:** `access` reads `"readWrite"` or `"readOnly"` correctly in each case; `unreadableCount` is `0`, `0`, and `1` respectively.

---

## Unlock screen

### T6.4 — The unlock screen shows four things and nothing else
*Covers P6.1.b · Verifies Design §5*

Render the unlock screen for a chosen vault path.
**Verdict:** the vault's name (derived from the path's final component), its full location, a password field, and an unlock button — and nothing else. No logo, tip, feature callout, or badge.

### T6.5 — The unlock button stays visibly alive during derivation
*Covers P6.1.c · Verifies Design §5, C-3*

Submit a correct password and observe the interface during the ~1 second Argon2id derivation takes.
**Verdict:** the button shows a working state; the password field is disabled; nothing resembling a percentage or a determinate bar is shown, since C-3's one second has nothing measurable to report against.

### T6.6 — A wrong password and a damaged vault are different screens
*Covers P6.1.d · Verifies Design §5, FR-2*

Attempt unlock with a wrong password against an intact vault; separately, attempt unlock against a vault with a damaged header (Phase 1's corruption technique).
**Verdict:** the wrong-password case shows exactly *"That password didn't work. Try again."* — no attempt count, no lockout, no hint about the password's shape. The damaged-vault case shows a visually distinct screen: *"This vault can't be read. It may be incomplete or damaged."* plus what is known, and advice to work from a backup.

### T6.7 — Directing a damaged-vault user to retry is never shown
*Covers P6.1.d · Verifies Design §5*

Inspect the damaged-vault screen of T6.6.
**Verdict:** nothing on it suggests re-entering the password or retrying unlock — the whole point of distinguishing the two screens is that retry delays acting on a backup.

### T6.8 — No "remember this password" exists anywhere
*Covers P6.1.e · Verifies Design §5, HC-7*

Inspect the unlock screen and its surrounding chrome (window menu, settings, if any exist by this phase) for a persistence offer.
**Verdict:** none found.

---

## Format version messages

### T6.9 — Format-too-new and superseded-format messages
*Covers P6.2.a, P6.2.b · Verifies Design §5, FR-5, FR-6*

Attempt to open a header stamped with a `format_version` above `CURRENT_FORMAT_VERSION` (too new); separately, one below `OLDEST_SUPPORTED_FORMAT_VERSION` (superseded) — both deliberately constructed, since no vault this codebase writes today falls into either case on its own.
**Verdict:** the too-new case names the version required and the version this release understands. The superseded case states the vault's format version, that it opens normally, and that a future release may offer to convert it — and the vault does, in fact, open (this is the one condition here that is not a refusal).

---

## Creating a vault

### T6.10 — Creating a vault names it, places it, and sets its password
*Covers P6.3.a · Verifies Design §8.2, FR-1*

Walk the create flow end to end: choose a location, type a password meeting C-4, acknowledge the disclosure, create.
**Verdict:** the vault exists at the chosen path afterward, empty, opening with the password just set.

### T6.11 — The unrecoverability disclosure is exact, acknowledged, and not skippable
*Covers P6.3.b, P6.3.e · Verifies Design §8.2, HC-7, FR-27*

Reach the disclosure step of vault creation.
**Verdict:** the block reads exactly *"If you forget this password, everything in this vault is lost. There is no recovery, no reset, and no way for anyone — including us — to get it back. Write the password down and keep it somewhere safe."*, styled as `caution`; acknowledgement is an explicit, unchecked control; there is no way to proceed without checking it and no "skip" affordance.

### T6.12 — A short password is refused before creation, and strength feedback never claims strength
*Covers P6.3.c, P6.3.d · Verifies C-4, Design §7*

Attempt to create a vault with an 11-character password; separately, with a long, high-entropy one.
**Verdict:** the short password is refused with `PasswordTooShort`'s message, both before the backend is called (client-side check) and by the backend itself if that check is bypassed. Nothing about the long password is described as "strong" or as resisting attack — only ever descriptive ("short" or nothing at all).

---

## Identity bar

### T6.13 — Lock state is legible at a glance and in greyscale
*Covers P6.4.a, P6.4.b · Verifies Design §3.2*

Render the identity bar unlocked, then render the equivalent locked-adjacent state (P6.9.d's separate screen also carries an identity-bar-consistent indicator).
**Verdict:** the two states differ in more than colour — shape, text, or icon as well — confirmed by desaturating a screenshot and checking the distinction still reads.

### T6.14 — The statistics line is correct and present immediately
*Covers P6.4.d · Verifies Design §3.2, FR-7*

Open a vault of a known entry count and total size.
**Verdict:** the statistics line shows the exact count and a human-readable size (§7: `312.4 GB`, not rounded) the instant the vault opens — no loading state, no delay proportional to vault size.

---

## Search and grouping

### T6.15 — Search filters the list; grouping is a flat view, not a tree
*Covers P6.5.a, P6.5.b · Verifies Design §3.2, §1.2, FR-8*

Type a search term matching some entries' names and others' folders; separately, toggle folder grouping.
**Verdict:** search narrows the list to matches on name or folder. Grouping collapses/expands by folder path with no control to create, rename, or drag a group, and no nesting beyond one level of the recorded (flat) folder string.

---

## Deleting

### T6.16 — Delete is confirmed by name or exact count, and takes effect immediately
*Covers P6.8.a, P6.8.b · Verifies Design §4.1, §8.4, FR-22*

Delete one entry; separately, select and delete several.
**Verdict:** the confirmation reads *"Delete {file}?"* for one, *"Delete {N} files?"* for several — never "Delete selected item?". Confirming removes them from the list at once, and they are unreachable afterward (`list_entries` no longer returns them; extraction by their old id fails with `NotFound`).

### T6.17 — Delete makes no claim about freed storage
*Covers P6.8.c · Verifies Design §8.4*

Inspect the delete confirmation and completion states.
**Verdict:** neither states or implies that space is reclaimed immediately — Spec §4.5's residue model means it may not be, and Design leaves this undesigned until a reclaim step exists to design around.

---

## Error presentation

### T6.18 — Every error renders in three parts, at the location of the action
*Covers P6.10.a, P6.10.b · Verifies Design §4.2*

Trigger a failure during an add, an extraction, and a delete.
**Verdict:** each renders at the site of that action (never a system notification) and states, in order: what happened, the vault's current state, and what can be done next. The current-state part is present in every case, not only the ones where it happens to be easy to add.

### T6.19 — Constrained conditions render their designed response, not a generic failure
*Covers P6.11.a–g · Verifies Design §4.3, FR-16, FR-23, FR-24, FR-25, S-3*

Reach each condition: open a vault already open elsewhere (`VaultInUse`); open one on read-only media (Phase 4's technique); force a `ChangedOnDisk` by reloading the vault from a second handle mid-session and writing there first; simulate `StorageUnavailable` by removing the backing volume mid-operation (Phase 4's crash-adjacent technique, run without a kill); exceed the per-vault entry limit; damage one entry's file and try to extract it.
**Verdict:** each shows exactly the response Design §4.3 designs for it — retry offered for in-use, no read-only fallback offered for it; the read-only banner's exact wording; reload offered (never silent merge) for changed-on-disk; the volume named for storage-unavailable; the limit and current value together for limit-exceeded; the damaged entry marked in the list rather than the whole vault failing.

---

## Adding files

### T6.20 — Both add paths reach the same place, and completion discloses the originals
*Covers P6.6.a, P6.6.b, P6.6.d · Verifies Design §3.3, §8.3, FR-27*

Add files by dropping them; separately, through an explicit "Add files" control. Drop a folder (`Photos/`) containing a file at its own top level (`b.jpg`) and a file nested under a sub-folder (`2024/a.jpg`).
**Verdict:** both produce the same entries. On completion: exactly *"Added {N} files. The originals are still on your disk — Veil doesn't delete them."* No control anywhere offers to delete the originals; a "reveal in file manager" affordance, if present, is fine. `b.jpg` stores with folder `"Photos"` and `a.jpg` with folder `"Photos/2024"` — the dropped folder's own name is the top-level segment, never discarded (FR-10).

### T6.21 — A cancelled add leaves the vault exactly as it was
*Covers P6.6.c · Verifies Design §4.1, FR-15*

Start adding a file large enough to be genuinely in flight; cancel partway.
**Verdict:** progress and a cancel control were visible throughout; afterward the vault's entries and statistics are identical to before the add started (the residue left on disk, if any, is not index-visible — Phase 2's own class of assertion).

---

## Extracting

### T6.22 — The destination is always chosen, and overwrite is confirmed by name
*Covers P6.7.a, P6.7.b · Verifies Design §6, FR-17, FR-19*

Extract a file to a destination that does not yet hold a file by that name; separately, to one that does.
**Verdict:** no default destination is offered either time — a dialog is always shown. The second case confirms by name before writing: *"Replace {file} in {folder}?"*.

### T6.23 — Success discloses the copy is unprotected, every time
*Covers P6.7.d · Verifies Design §6, FR-27*

Extract the same file twice, to two different destinations.
**Verdict:** both times, the completion state reads exactly *"Saved to {folder}. This copy is not protected."* — stated as text in the completion state, not a dismissable warning dialog, and not skipped the second time on the theory the user already knows.

---

## Locking

### T6.24 — Lock is one click away, and quitting locks
*Covers P6.9.a, P6.9.b · Verifies Design §3.2, §8.5, FR-3*

Lock from the identity bar while a vault is open; separately, quit the application with a vault open.
**Verdict:** one click reaches the lock action from anywhere the identity bar is visible. Quitting with a vault open leaves nothing unlocked on the next launch — there is no setting that changes this.

### T6.25 — Nothing locks automatically
*Covers P6.9.c · Verifies Design §8.5, FR-3*

Leave a vault open and idle past any interval that might plausibly trigger an automatic lock; separately, sleep and wake the machine; separately, engage the screen lock and return.
**Verdict:** the vault is still unlocked in all three cases. Nothing in the interface claims otherwise.

### T6.26 — The locked screen is not a greyed-out list
*Covers P6.9.d · Verifies Design §8.5, HC-1*

Lock a vault that was showing its entry list, and inspect the resulting screen.
**Verdict:** no file name, folder, size, or count from the list is visible or present in the DOM — a distinct screen, not the same list dimmed. (Mechanically checkable: the locked view's rendered markup contains none of the prior session's entry names.)

---

## Checking a vault for damage

### T6.27 — The check is user-initiated, estimates time, and reports per entry
*Covers P6.0.f, P6.12.a, P6.12.b · Verifies Design §8.6, FR-26*

Start a check on a vault of known size; observe what is shown before it starts and while it runs.
**Verdict:** nothing runs a check without this being explicitly started here. Before starting, a time estimate is shown (not a byte count). While running, progress is reported per entry, and a cancel control is present throughout.

### T6.28 — A cancelled check reports what it completed
*Covers P6.12.b · Verifies Design §8.6*

Cancel a check partway through.
**Verdict:** the result names how many entries were actually checked and whatever failures were found among those, rather than discarding the partial result or reporting nothing.

### T6.29 — A clean result is narrow; a failed result names files and states the limit plainly
*Covers P6.12.c, P6.12.d · Verifies Design §8.6, S-3*

Run a check against an intact vault; separately, against one with entries damaged by Phase 1's corruption technique.
**Verdict:** the clean case reads exactly *"Checked {N} files. No damage found."* — never a standing health claim. The failed case names every damaged file and states plainly, in words close to *"{N} files are damaged. Their data in this vault can't be recovered — Veil doesn't keep a spare copy. If you have a backup, restore these files from it."*, followed by the file list.

### T6.30 — Damaged entries stay marked after the check dialog closes
*Covers P6.12.e · Verifies Design §8.6, S-3*

Dismiss the result dialog of a failed check.
**Verdict:** the damaged entries are still visibly marked in the list afterward (P6.11.g's per-entry treatment), and every other entry remains usable and looks it.

---

## Replacing a file

**Superseded interaction, recorded rather than deleted from history:** the first version of this section (Design §8.7 v1.1) described detecting a drop "onto its row" from the webview's reported drag position. Built, then tested live: the position `DragDropEvent` reports did not correspond to the cursor's actual location, reproducibly, regardless of where in the window it was, and the standard DOM `dragover`/`drop` events that could have been a fallback never fired at all — the webview's own native handler consumes the OS-level drag before the page ever sees it. §8.7 is now v1.2; T6.33 below tests what actually ships.

### T6.33 — A collision is matched by folder and name together, never name alone
*Covers P6.16.b · Verifies Design §8.7, FR-14*

Add a file whose name matches an existing entry but whose folder does not (`FolderB/x.bin` when the vault holds `FolderA/x.bin`); separately, one whose folder *and* name both match.
**Verdict:** the first is added as a new, distinct entry — no collision reported. The second is reported as a collision (`Collision { path, name, folder }`), not added and not failed outright.

### T6.34 — Replace is confirmed, states the loss, and updates the added date, by either path
*Covers P6.16.c, P6.16.d, P6.16.e · Verifies Design §8.7, §4.1*

Select an entry and choose "Replace…", picking a source file with a different name than the entry's own; separately, drop or add a file whose folder and name collide with an existing entry.
**Verdict:** both reach a confirmation — *"Replace {file}? Its current content in this vault will be gone."* for one, *"Replace {N} files already in this vault? Their current content will be gone."* naming each for several — before anything changes. After confirming either way: the entry's name and folder are unchanged, its size and content match the new source, and its "added" figure reflects the replacement's time, not the original add's. The explicit "Replace…" path accepts a source file regardless of that file's own name; the identity-matched path does not need to, since it exists specifically because the names already matched.

### T6.37 — A collision partway through a dropped folder does not cost the rest of it
*Covers P6.6.e, P6.16.b · Verifies FR-10, FR-14*

Drop a folder containing ten files, one of which collides with an existing entry.
**Verdict:** the other nine are added; the tenth is reported as a collision, not silently dropped and not aborting the other nine — `Vault::add_folder` would return on the first error, which is why `add_files` walks the folder itself rather than calling it.

---

## Changing the password

### T6.35 — Change-password is reached from the identity bar and mirrors the CLI
*Covers P6.17.a, P6.17.b · Verifies HC-5, Design §8.8, §2.4*

Open the change-password control from the identity bar; change the password through it.
**Verdict:** a labelled control, not an icon. Afterward, the vault opens with the new password and not the old one; the header's recorded KDF parameters are `for_new_vaults()`'s, freshly written, not carried over from the old header.

### T6.36 — Change-password requires the current password, validates the new one twice, and discloses the risk
*Covers P6.17.c, P6.17.d, P6.17.e · Verifies Design §8.8, HC-7, C-4, §7*

Attempt to change the password: with the wrong current password; with a new password under 12 characters; with two different spellings of the new password; then correctly.
**Verdict:** the wrong current password is refused (`WrongPassword`) before any change happens. The short new password is refused with `PasswordTooShort`'s message, client-side, before the backend is called. Mismatched retyping is caught before submission. On the correct attempt, the disclosure read exactly *"If you forget the new password, this vault is lost the same way it would be with the old one — there is still no recovery."*, with no claim about the new password's strength.

---

## Vocabulary and platform

### T6.31 — The vocabulary audit is clean in both applications
*Covers P6.13.a, P6.13.b, P6.13.c · Verifies Design §1.2, §7*

Grep every user-facing string in `crates/veil-gui/ui` and every format string in `crates/veil-cli/src` against Design §7's denylist (entry, object, item, blob, record, container, archive, repository, volume, directory, passphrase, master password, key, import, ingest, encrypt, export, decrypt, download, unlock, close, seal, verify, validate, integrity check, scrub, fsck) and its forbidden-claims list ("military-grade", "bank-level", "unbreakable", "100% secure", "hacker-proof", "your data is safe"), and against §1.2's anti-goal vocabulary (padlock, shield, keyhole).
**Verdict:** none found outside these two documents and the source code's own internal comments (which Design §7 explicitly permits to use internal terms). A later string that reaches for one of these fails this case.

### T6.32 — The release states its platform
*Covers P6.15.a · Verifies Requirements §2.1, §8*

Inspect the release's own materials (about screen, README, release notes — whichever exist by this point).
**Verdict:** macOS is named as what 2.0.0 was built for and run on; nothing claims Windows or Linux support, "coming soon" or otherwise.

---

## Coverage

| Identifier | Case |
|---|---|
| HC-1 | T6.26 |
| HC-5 | T6.2 |
| HC-7 | T6.8, T6.11 |
| C-3 | T6.5 |
| C-4 | T6.12 |
| S-2 | T6.14 |
| S-3 | T6.3, T6.19, T6.29, T6.30 |
| FR-1 | T6.2, T6.10 |
| FR-2 | T6.6, T6.7 |
| FR-3 | T6.24, T6.25 |
| FR-5, FR-6 | T6.9 |
| FR-7 | T6.14 |
| FR-8 | T6.15 |
| FR-9 | T6.20 |
| FR-10 | T6.20, T6.33, T6.37 |
| FR-13 | T6.34 |
| FR-14 | T6.33, T6.37 |
| FR-15 | T6.21 |
| FR-16 | T6.19 |
| FR-17, FR-19 | T6.22 |
| FR-18 | (P6.7.c — covered by Phase 1's existing damaged-extraction cases; not re-verified here) |
| FR-22 | T6.16 |
| FR-23 | T6.3, T6.19 |
| FR-24, FR-25 | T6.19 |
| FR-26 | T6.27 |
| FR-27 | T6.11, T6.20, T6.23 |

**Not reachable in Phase 6**: signing and notarisation (P6.14, out of scope for this document).
