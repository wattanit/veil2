# Veil2 — Design Guideline

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-09
**Owner:** wattanit
**Companion documents:**
- Requirements Document v1.0 — upstream
- Technical Specification v1.0 — downstream

This document specifies how Veil2 looks, feels, and communicates: identity and anti-goals, visual language, layout, interaction and response policy, the wording of every honesty clause FR-27 requires, and the moments that determine whether the product is trusted. It defers what the product must do to the Requirements Document, and how it is built to the Technical Specification. Working values below — colors, sizes, thresholds — are initial and tunable; they are specified so that tuning is a value change rather than a redesign.

---

## 1. Identity

### 1.1 Concept

The interface is a browsable list of files, not a single encrypted block that must be fully decrypted to use. The primary user action is finding one file in the list and retrieving it.

The file list is the primary interface. Encryption makes the list's contents trustworthy, but is not itself part of the user-facing interaction. Design choices prioritize the list over visual emphasis on cryptography.

### 1.2 Anti-goals

Each of the following is prohibited, with its reason.

**No security theater.** No padlock iconography, shields, keyholes, dark-and-green "hacker" color schemes, or language such as "military-grade encryption" or "unbreakable." Requirements §7 states that Veil2 does not defend against a compromised host; visual claims of impregnability would overstate this protection. Confidence is communicated through precise language, not through visual signaling of strength.

**Not a file manager.** Veil2 does not provide rename-in-place, folder creation, move-between-folders, or in-app preview. Storage is flat (FR-8); a UI implying a directory tree would imply operations the format does not support. Grouping the list by recorded folder path (§3.2) is not an exception to this: it is a flat view control with no create, rename, or drag, and no nesting.

**Not a sync client.** No operation runs in the background. Verification is user-initiated, not automatic (FR-26). Any operation in progress was started by the user and can be canceled by the user.

**No progress theater.** No indeterminate spinners for measurable work, no progress bars that advance on a timer rather than actual progress, no percentage that stalls before completion. Progress values reflect actual work completed; at the sizes Veil2 targets, the user is deciding whether to wait or cancel based on this number.

**No reassurance by default.** Where a guarantee has a limit, the limit is stated at the point of use (FR-27), not only in documentation. The product does not present confirmatory messaging about security.

---

## 2. Visual Language

### 2.1 Principles

The file list is the primary visual element. Chrome is minimized. Any interface element that is not a file, its metadata, or a control acting on it requires justification.

Visual restraint serves a functional purpose: in a dense list of thousands of entries, color and weight are the primary means of signaling a destructive or in-progress state. Using them decoratively reduces their effectiveness for that purpose.

### 2.2 Color

Working values, tunable, defined once and referenced everywhere. Light and dark are both supported and follow the system setting.

| Token | Role | Light | Dark |
|---|---|---|---|
| `surface` | Application background | `#FFFFFF` | `#1C1C1E` |
| `surface-raised` | Toolbar, footer, dialogs | `#F5F5F7` | `#2C2C2E` |
| `text` | Primary text | `#1C1C1E` | `#F2F2F7` |
| `text-muted` | Metadata, secondary | `#6E6E73` | `#98989D` |
| `border` | Separators, list rules | `#D8D8DC` | `#3A3A3C` |
| `accent` | Selection, in-progress | `#2F6FEB` | `#4C8DFF` |
| `caution` | Destructive and irreversible | `#C4341C` | `#FF6B52` |

Two semantic colors are defined: `accent` and `caution`. No success color is defined. Successful completion is the expected outcome and does not require visual emphasis; a green indicator next to a file would imply a security claim ("this file is safe") that Veil2 does not make.

`caution` is used only for irreversible actions: deleting an entry, overwriting a destination file, creating a vault (whose password cannot be recovered, per HC-7). It is not used for other warnings.

### 2.3 Typography and density

System UI font throughout. Sizes are tunable: body `13px`, list rows `13px`, secondary metadata `11px`, headings `15px` semibold.

Numerals in size and date columns use tabular figures, so digits align vertically and magnitudes can be compared by scanning. This rule is fixed and does not change with other tunable values.

List rows are `28px`, dense by default. Cards, thumbnails, and large padding are not used; the design goal is maximizing the number of entries visible per screen, since a vault may hold thousands of entries.

Monospace is not used in the interface.

### 2.4 Iconography

Icons are used only where they communicate faster than text: drag-target affordance, lock state, per-row type indication. All other controls are labeled with text. Icon-only toolbars are not used, since misinterpreting a consequential action is costly.

---

## 3. Layout

### 3.1 Single panel

One panel shows vault contents. There is no second panel for the local filesystem; the operating system's file manager serves that function, connected via drag-and-drop and Save-As. A second panel displaying vault contents alongside a local filesystem view would also imply an unencrypted region within the vault, which HC-1 prohibits.

```
┌────────────────────────────────────────────────────────────┐
│  ▣ Holiday Photos        Unlocked          [ Lock ]        │  ← identity bar
│  1,284 files · 312.4 GB stored                              │  ← statistics (FR-7)
├────────────────────────────────────────────────────────────┤
│  [ search ]                    Group by folder ▾   [ + ]   │  ← controls
├────────────────────────────────────────────────────────────┤
│  Name                    Folder         Size       Added   │
│  ─────────────────────────────────────────────────────────  │
│  IMG_4417.raw            2024/Iceland   48.2 MB   12 Mar   │  ← content list
│  IMG_4418.raw            2024/Iceland   47.9 MB   12 Mar   │
│  …                                                          │
├────────────────────────────────────────────────────────────┤
│  Extracting IMG_4417.raw…  ▓▓▓▓▓▓░░░░  62%  4.2 MB/s [ ✕ ] │  ← operation bar
└────────────────────────────────────────────────────────────┘
```

*Illustrative example; exact proportions and wording are not specified here.*

### 3.2 Regions

**Identity bar** — vault name, lock state, lock action. Lock state must be identifiable at a glance and at a distance. The locked and unlocked states differ in more than color, so the distinction remains identifiable for color-blind users and in greyscale.

**Statistics line** — entry count and total stored size, always visible (FR-7). Displaying this immediately at open has no performance cost, since open time does not scale with vault size (S-2).

**Controls** — search, the grouping toggle, and add. Grouping by recorded folder path is a *view* control, not a directory tree (FR-8). Groups can be collapsed and expanded; they cannot be renamed, created, or dragged, since folder path is metadata rather than structure.

**Content list** — virtualized, sortable by any column, multi-select. Columns are **name, folder, size, added**, in that order; §3.4 specifies the same order for the command-line output. This order places name first (the primary search target), folder second (disambiguates same-named files), then size and date (comparison values, formatted per the tabular-numeral rule in §2.3).

**Operation bar** — present only while an operation is running. Shows what is happening, actual progress, throughput, and a cancel control. One operation is visible at a time; queued work is stated as a count.

### 3.3 Drop target

With a vault open, the entire window functions as a drop target. On drag-enter, the drop affordance states the resulting action — for example, "Add 34 files to Holiday Photos" — so the file count is visible before the user releases.

With no vault open, dropping a vault opens it. Dropping anything else is refused with an explanation.

### 3.4 Command-line surface

The CLI is a peer application, not a debug tool (A-4). Requirements:

- Default output is a table with the same column order as the GUI.
- Machine-readable output is available on request for scripting; the human-readable default is not machine-formatted.
- Progress is written to standard error, results to standard output.
- Progress rendering degrades to periodic lines when not attached to a terminal, rather than emitting control characters into a log.
- The CLI does not prompt when a non-interactive invocation is detected; it fails with a message naming the missing input.

---

## 4. Interaction and Response Policy

### 4.1 Standing rules

**Every long operation is cancellable, and cancellation states its result.** An interrupted add leaves the vault as though it had not started (FR-15); an interrupted extraction leaves no usable partial file (FR-18). The result is stated explicitly.

**No silent retry, no silent degradation.** If an operation cannot proceed, it stops and reports why. Automatic retry would obscure a failing drive, which the user needs to know about.

**Confirmation is required only for irreversible actions**: deleting an entry, overwriting a destination file, creating a vault. No other action requires confirmation. Frequent confirmation prompts reduce attention to the prompts that matter.

**Confirmations name the object.** "Delete IMG_4417.raw?" not "Delete selected item?" Where a count is involved, it is exact: "Delete 34 files?"

### 4.2 Failure

Errors have three parts, in this order: **what happened**, **the current state**, **available actions**. The current-state part is often omitted and is the most important here, since the user's primary concern after a failed write to an encrypted vault is whether the vault is still intact.

Errors are shown at the location of the action. A failure during extraction appears at the extraction, not as a system notification.

### 4.3 Constrained conditions

Each of these is an expected condition with a designed response, not an exception path.

**Vault in use by another process (FR-23).** State that it is open elsewhere and offer to retry. Do not offer to open read-only instead: FR-23 does not provide a read-only path around genuine lock contention, only for storage that cannot take a lock at all (the next condition). Do not offer to break the lock either: the lock exists to prevent two writers from corrupting the vault.

**Vault open, but nothing can be written to it (FR-23).** The vault is on read-only media, its storage does not support locking, or the user cannot write to the directory. It opens, and all read operations function normally — browsing, saving copies, checking for damage. The state is stated when the vault opens, in the identity bar alongside the lock state, rather than discovered when a write fails. The wording distinguishes this from damage or failure:

> "Read-only — this vault can't be changed from here."

Controls that would write are disabled rather than hidden, with the reason available on the control.

**Vault changed on disk since opening (FR-24).** Stop before writing. State that something external changed the vault, and offer to reload. Never merge or overwrite silently. Reconciling divergence is out of scope (Requirements §2.3), and the design does not imply that Veil2 can do this.

**Storage disappeared mid-operation (FR-25).** Name the volume that became unavailable, state that the vault is intact as of the last completed step, and return to a usable state without requiring a restart.

**Destination full during extraction.** Remove the partial file. State how much space was needed against how much was available.

**Vault full or file too large (FR-16).** Name the limit and the current value in the same message.

**A file is in a damaged region (S-3).** Mark the affected entries in the list rather than failing the whole vault. The rest of the vault remains usable, which S-3 requires; the interface reflects this rather than presenting a general failure.

---

## 5. The Unlock

This is the primary gate in the product and the screen every user sees on every use.

**It shows four things and nothing else:** the vault's name, its location on disk, a password field, and an unlock button. No branding, tips, feature tour, or security badge.

**Key derivation takes approximately one second (C-3); the interface must not appear frozen during this time.** The unlock button shows a determinate-looking working state and the field locks during derivation. No fake percentage is shown, since there is nothing measurable to display, but the interface must appear active.

**Wrong password and damaged vault are different screens.** FR-2 requires the distinction:

> *Wrong password* — "That password didn't work. Try again." No attempt count, no lockout, no hint about the password's shape.
>
> *Damaged vault* — "This vault can't be read. It may be incomplete or damaged." Followed by what is known, and advice to work from a backup copy.

Directing a user with a damaged vault to retry their password delays the point at which they could act on a backup.

**A vault in a superseded format states this plainly** (FR-6): which format version it uses, that it opens normally, and that a future release may offer to convert it. A vault too new to read (FR-5) names the version required.

**There is no "remember this password" in v1.** HC-7 makes the password the only safeguard against permanent loss; storing it in an OS keychain would relocate the security boundary to the keychain. If offered in a future version, this must be stated explicitly.

---

## 6. The Extraction

Extraction is the only path by which plaintext leaves a vault (HC-2, FR-17).

**The destination is always chosen and always shown.** There is no default download folder and no one-click extract. The user names the destination each time.

**Overwrites are confirmed by name** (FR-19): "Replace IMG_4417.raw in Pictures?"

**Verification failure removes the output** (FR-18). The message states that the file was damaged in the vault, that the incomplete copy has been removed, and that the vault's other files are unaffected.

**Success states the consequence, once, plainly:** "Saved to Pictures. This copy is not protected." Stated as a line of text in the completion state, not a warning dialog. This is stated every time, since an extracted file that the user forgets is unprotected is a common route by which data leaves Veil2's protection.

---

## 7. Voice and Language

This document follows the rules below in its own writing.

**Plain words.** Short sentences. Second person. No hedging.

**Fixed vocabulary — one word per thing, everywhere, GUI and CLI alike:**

| Use | Never |
|---|---|
| vault | container, archive, repository, volume |
| file | entry, object, item, blob, record |
| folder | directory, path (as a noun in the UI) |
| password | passphrase, master password, key |
| add | import, ingest, encrypt |
| save a copy | export, decrypt, download, unlock |
| lock | close, seal |
| check for damage | verify, validate, integrity check, scrub, fsck |

Internal terms — entry, ingest, master key — belong in these documents and the source code, not on screen. The user-facing verb for retrieval is "save a copy" rather than "decrypt": it describes the effect from the user's perspective, including that a second, unprotected copy now exists.

**Forbidden words and claims:** "military-grade", "bank-level", "unbreakable", "100% secure", "hacker-proof", "your data is safe." Requirements §7 lists what Veil2 does not protect against; these claims contradict it.

**Errors do not blame the user and do not expose internals.** No error codes in primary text, no cryptographic library messages, no stack traces. A detail view may carry technical text for a bug report; the first sentence is always in plain language.

**Numbers:** human-readable by default (`312.4 GB`), exact bytes on hover. Counts are exact, never rounded — "1,284 files", not "about 1,300".

**Honesty clauses are direct and specific.** Not "vault size may be observable" but "anyone who has this file can see how large it is and roughly when you last changed it."

---

## 8. Key Moments

### 8.1 First run

No vault exists. The window offers exactly two choices — create a vault, or open one — and nothing else. No tour, sample vault, or account.

### 8.2 Creating a vault

HC-7 (password loss is unrecoverable) is disclosed here.

The user names the vault, chooses a location, and sets a password subject to C-4. Before the vault is created, a `caution` block states:

> **If you forget this password, everything in this vault is lost.** There is no recovery, no reset, and no way for anyone — including us — to get it back. Write the password down and keep it somewhere safe.

Confirmation is an explicit acknowledgment, not a pre-checked checkbox. A password manager may be suggested; a "skip" option is not offered.

Strength feedback is descriptive, not permissive: it may state that a password is short, but does not state that a password is "strong," since that would be a claim about attacker resources that cannot be verified.

### 8.3 Adding files

On completion, the disclosure FR-27 requires appears once, in the completion state:

> "Added 34 files. The originals are still on your disk — Veil doesn't delete them."

Offering to reveal the originals in the file manager is appropriate. Offering to delete them is not: Requirements §2.3 places secure erasure out of scope, and a delete control here would imply a guarantee Veil2 cannot provide.

### 8.4 Deleting

Deletion is confirmed with `caution` and names the files:

> "Delete 12 files?"

Deletion is immediate: deleted files leave the list and become unreachable (FR-22). Whether the underlying storage is freed immediately or requires a separate step is a Technical Specification decision, not addressed here. If a separate step exists, its presentation is designed when that decision is made.

### 8.5 Locking and ending

Locking is explicit and always one click from the identity bar. Quitting the application locks the vault; there is no "leave it open" setting.

**The vault is not locked automatically** — not on inactivity, system sleep, or screen lock (FR-3). This is an explicit design decision: an unexpected password prompt during a working session trains users to enter their password without verifying the prompt's source, which is a greater risk than the benefit of an automatic lock.

An unlocked vault on an unattended machine remains readable. This is not obscured in the design: the lock-state indicator in §3.2 is legible at a distance specifically so that "this vault is open" is never a surprise. No copy anywhere implies that Veil2 locks itself when the user steps away; Requirements §7 lists this among the things it does not do.

The locked state is a distinct screen, not a greyed-out list. A visible file list while locked would suggest the index is still readable, which is the confusion HC-1 exists to prevent.

---

### 8.6 Checking a vault for damage

Presented as maintenance the user chooses, never on a schedule (FR-26). It reads the whole vault, so the control states an estimate in time (not a byte count) before starting, since the decision being made is whether to wait.

Progress is reported per entry, and cancellation is available throughout. A canceled check reports what it completed rather than discarding the result.

**A clean result is stated narrowly.** Not "your vault is healthy" — a standing claim that does not hold for a check that describes one point in time. The wording describes what was done:

> "Checked 1,284 files. No damage found."

**A failed result names the files and states what cannot be done**, since users may otherwise assume Veil can repair the damage:

> **3 files are damaged.** Their data in this vault can't be recovered — Veil doesn't keep a spare copy. If you have a backup, restore these files from it.
>
> `IMG_4417.raw` · `IMG_4418.raw` · `notes/2019.md`

Damaged entries are marked in the list afterward, using the same treatment as §4.3, so the result remains visible after the dialog is dismissed. All other files remain usable (S-3); this is visually apparent rather than presenting the vault as broadly compromised.

---

## 9. Open Questions

- **Whether the damage check of §8.6 is reachable from the identity bar or only from a menu.** Resolver: Design Guideline, next version.
- **Whether the folder-grouping view (§3.2, §1.2) is on or off by default.** Depends on whether real vaults tend to have many distinct folder-path values, which grouping helps navigate, or few, where grouping adds no value. This does not reopen whether grouping is a tree — it is not (§1.2) — only its default state. Resolver: tune with use, once real vaults exist.
- **Whether the CLI's user-facing strings are shared with the GUI's or maintained separately.** §7 requires identical vocabulary; whether this is enforced mechanically or by review is a build question. Resolver: Technical Specification.
- **Palette values in §2.2 against real content.** Chosen for contrast on paper, not yet checked against dense lists of long filenames in both themes. Resolver: tune with use.
- **Application icon and installer identity.** Not yet designed; §1.2 excludes padlocks and shields, which rules out most conventional icons for this category. Resolver: Design Guideline, next version.
- **Whether search covers folder metadata as well as filenames**, and whether it is literal or fuzzy. Resolver: Design Guideline, next version, informed by entry counts in real vaults.
