# Veil2 — Design Guideline

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-08
**Owner:** wattanit
**Companion documents:**
- Requirements Document v1.0 — upstream
- Technical Specification v1.0 — downstream

This document owns how Veil2 looks, feels, and speaks: identity and anti-goals, visual language, layout, interaction and response policy, the wording of every honesty clause FR-29 requires, and the moments that decide whether the product is trusted. It defers what the product must do to the Requirements Document, and how it is built to the Technical Specification. Working values below — colours, sizes, thresholds — are initial and tunable; they are given so that tuning is a value change rather than a redesign.

---

## 1. Identity

### 1.1 Concept

Veil2 is **a filing cabinet with a lock, not a safe you must empty to use.** Everything about the design follows from that: the user's time is spent reading a list and pulling one thing out of it, not performing an unlock-everything ritual.

The index is the product. Encryption is what makes the index trustworthy, but the user's actual experience is scanning, finding, and retrieving. A design that foregrounds cryptography over the list has misunderstood what people do here.

### 1.2 Anti-goals

What the design must not become is as normative as what it must be. Each of these is forbidden, with its reason.

**No security theatre.** No padlock iconography, no shields, no keyholes, no dark-and-green "hacker" palette, no "military-grade encryption" or "unbreakable" language anywhere in the product. The reason is direct: §7 of the Requirements says Veil2 does nothing against a compromised host, and a product that dresses itself as impregnable manufactures exactly the over-trust that gets people hurt. Confidence is communicated by being precise, not by looking fortified.

**Not a file manager.** Veil2 does not compete with Finder or Explorer. It offers no rename-in-place, no folder creation, no move-between-folders, no in-app preview. Storage is flat (FR-7); a UI implying a real directory tree would promise operations the format cannot honour.

**Not a sync client.** Nothing happens in the background. No auto-compaction (FR-23), no background verification, no ambient progress in a corner. If work is happening, the user started it and can stop it.

**No progress theatre.** No indeterminate spinners standing in for work that could be measured, no progress bars that advance on a timer, no percentage that reaches 99% and stalls. At the sizes in play a user is deciding whether to wait twenty minutes or cancel, and that decision requires a number that means something.

**No reassurance by default.** Where a guarantee has a limit, the limit is stated at the point of use, not buried in documentation (FR-29). The product does not congratulate the user on being secure.

---

## 2. Visual Language

### 2.1 Principles

The file list is the only hero. Chrome recedes. Any pixel that is not a file, its metadata, or a control acting on it must justify itself.

Restraint is a security property here, not an aesthetic preference: in a dense list of thousands of entries, colour and weight are the only tools left for signalling that something is destructive or in progress. Spending them on decoration disarms them.

### 2.2 Colour

Working values, tunable, defined once and referenced everywhere. Light and dark are both first-class and follow the system setting.

| Token | Role | Light | Dark |
|---|---|---|---|
| `surface` | Application background | `#FFFFFF` | `#1C1C1E` |
| `surface-raised` | Toolbar, footer, dialogs | `#F5F5F7` | `#2C2C2E` |
| `text` | Primary text | `#1C1C1E` | `#F2F2F7` |
| `text-muted` | Metadata, secondary | `#6E6E73` | `#98989D` |
| `border` | Separators, list rules | `#D8D8DC` | `#3A3A3C` |
| `accent` | Selection, in-progress | `#2F6FEB` | `#4C8DFF` |
| `caution` | Destructive and irreversible | `#C4341C` | `#FF6B52` |

Two semantic colours only — `accent` and `caution`. There is no success green: completing an operation is the expected case and needs no celebration, and a green tick next to a file invites the reading "this file is safe," which is a claim Veil2 does not make.

`caution` is reserved for actions that cannot be undone: deleting an entry, overwriting a file at a destination, creating a vault whose password cannot be recovered. Using it for ordinary warnings devalues it.

### 2.3 Typography and density

System UI font throughout. Sizes are tunable: body `13px`, list rows `13px`, secondary metadata `11px`, headings `15px` semibold.

**Numerals in size and date columns are tabular**, so digits align vertically and a user can compare magnitudes by scanning rather than reading. This is the one typographic rule that is not negotiable at any size.

List rows are `28px` and dense by default. Cards, thumbnails, and generous padding are wrong here: a media vault holds thousands of entries and the design goal is how many the user can assess per screen.

Monospace appears nowhere. It signals "technical output" and this is not that.

### 2.4 Iconography

Icons are used only where a word would be slower: drag-target affordance, the lock state, and per-row type indication. Everything else is a labelled control. An icon-only toolbar is forbidden — this product's actions are consequential and ambiguity is expensive.

---

## 3. Layout

### 3.1 Single panel

One panel showing vault contents. There is no second panel for the local filesystem: the operating system's own file manager already is that panel, and drag-and-drop plus Save-As spans the gap. A second panel would also have implied a plaintext region inside the vault, which HC-1 forbids.

```
┌────────────────────────────────────────────────────────────┐
│  ▣ Holiday Photos        Unlocked          [ Lock ]        │  ← identity bar
│  1,284 files · 312.4 GB stored · 18.2 GB reclaimable       │  ← statistics (FR-8)
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

*Example only; proportions and wording are illustrative and carry no normative weight.*

### 3.2 Regions

**Identity bar** — vault name, lock state, lock action. Lock state is the most important single fact in the application and must be legible at a glance from across a desk. Locked and unlocked differ in more than colour, so the distinction survives colour-blindness and greyscale screenshots.

**Statistics line** — the FR-8 figures, always visible, never behind a menu. They are the input to the compaction decision (FR-23) and a user who must go looking for them will not make that decision at all. Reclaimable space is shown plainly whenever it is non-trivial; the threshold for calling attention to it is tunable, initially 10% of physical size.

**Controls** — search, the grouping toggle, and add. Grouping by recorded folder path is a *view* control, not a tree widget (FR-7). Groups collapse and expand; they cannot be renamed, created, or dragged, because they are metadata rather than structure.

**Content list** — virtualised, sorted by any column, multi-select.

**Operation bar** — present only while an operation runs. Shows what is happening, real progress, throughput, and cancel. One operation at a time is visible; queued work is stated as a count.

### 3.3 Drop target

With a vault open, the entire window is a drop target. The drop affordance appears on drag-enter and names what will happen — "Add 34 files to Holiday Photos" — because the consequence of a drop is a copy into an encrypted store and the user should see the count before releasing.

With no vault open, dropping a vault opens it. Dropping anything else is refused with an explanation rather than ignored silently.

### 3.4 Command-line surface

The CLI is a peer, not a debug tool (A-4). Its design obligations:

- Default output is a table with the same column order as the GUI, so the two produce the same mental model.
- Machine-readable output is available on request for scripting; the human default is never machine-shaped.
- Progress is written to standard error, results to standard output, so pipelines are not polluted.
- Progress rendering degrades to periodic lines when not attached to a terminal, rather than emitting control characters into a log.
- It never prompts when a non-interactive invocation can be detected; it fails with a message naming the missing input.

---

## 4. Interaction and Response Policy

### 4.1 Standing rules

**Every long operation is cancellable and every cancel is honest.** Cancelling states what happened: an interrupted add leaves the vault as though it had not started (FR-14); an interrupted extraction leaves no usable partial file (FR-17). The user is told which, not left to guess.

**No silent retry, no silent degradation.** If an operation cannot proceed, it stops and says so. Automatic retry hides a failing drive, which is the condition the user most needs to learn about.

**Confirmation is reserved for the irreversible.** Deleting an entry, overwriting a destination file, and creating a vault are confirmed. Nothing else is. A product that asks "are you sure?" routinely trains people to click through the one prompt that mattered.

**Confirmations name the object.** "Delete IMG_4417.raw?" not "Delete selected item?" Where a count is involved it is exact: "Delete 34 files?"

### 4.2 Failure

Errors have three parts, always in this order: **what happened**, **what state things are in now**, **what you can do**. The middle part is the one usually omitted and the one that matters most here, because the user's real question after a failed write to an encrypted vault is whether the vault is still good.

Errors are shown where the action was taken. A failure during extraction appears at the extraction, not as a system notification.

### 4.3 Constrained conditions

Each of these is an expected condition with a designed response, not an exception path.

**Vault in use by another process (FR-26).** State that it is open elsewhere and offer to open read-only if that is possible, or to retry. Do not offer to break the lock: the lock exists because two writers corrupt the vault.

**Vault changed on disk since opening (FR-27).** Stop before writing. Explain that something else — most likely a sync client — changed the vault, and offer to reload. Never merge, never overwrite silently. Reconciling divergence is out of scope (§2.3 of Requirements) and the design must not imply Veil2 can do it.

**Storage disappeared mid-operation (FR-28).** Name the volume that went away, state that the vault is intact as of the last completed step, and return to a usable state without restarting the application.

**Destination full during extraction.** Remove the partial file, say how much space was needed against how much was free.

**Vault full or file too large (FR-15).** Name the limit and the current value in the same sentence.

**A file lives in a damaged region (S-4).** Mark the affected entries in the list rather than failing the whole vault. The rest of the vault remains fully usable, which is the entire point of S-4, and the UI must show that clearly instead of presenting a general alarm.

---

## 5. The Unlock

This is the most important screen in the product. It is the only gate, it is where trust is either established or lost, and it is the screen every user sees every time.

**It shows four things and nothing else:** the vault's name, its location on disk, a password field, and an unlock button. No branding, no tips, no feature tour, no security badge.

**It takes about a second and must not look broken.** Key derivation is deliberately expensive (C-3). During it the button becomes a determinate-looking working state and the field locks. It does not show a fake percentage — there is nothing meaningful to measure — but it must be visibly alive, because a one-second freeze on a password field reads as a hang.

**Wrong password and damaged vault are different screens.** FR-2 requires the distinction and the design carries it:

> *Wrong password* — "That password didn't work. Try again." No count of attempts, no lockout, no hint about the password's shape.
>
> *Damaged vault* — "This vault can't be read. It may be incomplete or damaged." Followed by what is known, and by the advice to work from a backup copy rather than this one.

Sending a user with a corrupted vault to retype their password wastes the time in which a backup might still exist. This distinction is worth the design cost.

**A vault in a superseded format says so plainly** (FR-30): which format version it uses, that it will open normally, and that a future release may offer to convert it. A vault too new to read (FR-5) names the version needed.

**There is no "remember this password" in v1.** HC-7 makes the password the only thing between the user and permanent loss, and storing it in an OS keychain silently relocates the security boundary to the keychain. If it is ever offered it must say exactly that, in those terms.

---

## 6. The Extraction

Extraction is the only path by which plaintext leaves a vault (HC-2, FR-16), which makes it the second moment worth designing deliberately.

**The destination is always chosen and always shown.** There is no default download folder and no one-click extract. The user names the destination every time, because Veil2's responsibility ends there and the user must know where it ended.

**Overwrites are confirmed by name** (FR-18): "Replace IMG_4417.raw in Pictures?" The original Veil overwrote silently, and a failed extraction destroyed the user's only good copy.

**Verification failure removes the output** (FR-17). The message says the file was damaged in the vault, that the incomplete copy has been removed, and that the vault's other files are unaffected.

**Success states the consequence, once, plainly:** "Saved to Pictures. This copy is not protected." Not a warning dialog, not a red banner — a line of text in the completion state. It is said every time because it is true every time, and a user who extracts a file and forgets it is unprotected is the most likely way data leaks out of Veil2.

---

## 7. Voice and Language

The rules below are followed by this document's own prose; a guideline that writes in a voice it forbids is not usable.

**Plain words.** Short sentences. Second person. No hedging, and no ceremony.

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
| reclaim space | compact, vacuum, garbage-collect |

Internal terms — entry, ingest, compaction, master key — belong in these documents and the source code, never on screen. The user-facing verb for retrieval is deliberately "save a copy" rather than "decrypt": it describes what happens to the user's world, and it carries the fact that a second, unprotected copy now exists.

**Forbidden words and claims:** "military-grade", "bank-level", "unbreakable", "100% secure", "hacker-proof", "your data is safe". The last is the most tempting and the most wrong — Requirements §7 lists six things Veil2 does not protect against.

**Errors never blame the user** and never expose internals. No error codes in primary text, no cryptographic library messages, no stack traces. A detail view may carry the technical text for a bug report; the first sentence is always human.

**Numbers:** human-readable by default (`312.4 GB`), exact bytes on hover. Counts are exact and never rounded — "1,284 files", never "about 1,300".

**Honesty clauses are direct and specific.** Not "vault size may be observable" but "anyone who has this file can see how large it is and roughly when you last changed it."

---

## 8. Key Moments

### 8.1 First run

No vault exists. The window offers exactly two choices — create a vault, or open one — and nothing else. No tour, no sample vault, no account.

### 8.2 Creating a vault

This is where HC-7 is stated, and it is the one place the product is deliberately uncomfortable.

The user names the vault, chooses a location, and sets a password subject to C-4. Before the vault is created, a single `caution` block states, without softening:

> **If you forget this password, everything in this vault is lost.** There is no recovery, no reset, and no way for anyone — including us — to get it back. Write the password down and keep it somewhere safe.

Confirmation is an explicit acknowledgement, not a pre-ticked checkbox. Suggesting a password manager here is appropriate; a "skip" affordance is not.

Strength feedback is descriptive, never permissive-sounding: it may say a password is short, but it never says one is "strong", because that is a promise about an attacker's resources that nobody can make.

### 8.3 Adding files

On completion, the honesty clause FR-29 requires appears once, in the completion state:

> "Added 34 files. The originals are still on your disk — Veil doesn't delete them."

Offering to reveal the originals in the file manager is appropriate. Offering to delete them is not: Requirements §2.3 puts secure erasure out of scope, and a delete button here would imply a thoroughness Veil2 cannot deliver.

### 8.4 Deleting and reclaiming space

Deletion is confirmed with `caution`, names the files, and states the limit plainly:

> "Delete 12 files? They'll be removed from the list immediately, but their data stays in the vault file until you reclaim space."

Reclaiming space is presented as maintenance the user chooses, with the FR-8 figures in the button itself — "Reclaim 18.2 GB" — so the decision needs no arithmetic. During it, the vault stays usable and the operation stays cancellable (FR-24).

### 8.5 Locking and ending

Locking is explicit and always one click from the identity bar. Quitting the application locks the vault; there is no "leave it open" preference.

**Nothing else locks the vault** — not inactivity, not system sleep, not the screen locking (FR-3). This is a written prohibition rather than an unbuilt feature, and it stays forbidden for a reason: an unexpected password prompt in the middle of a working session teaches people to retype their password without reading what asked for it, and that reflex is worth more to an attacker than the idle timer was worth to the user.

The consequence is that a vault left unlocked on an unattended machine stays readable, and the design must not paper over it. The lock-state indicator of §3.2 carries this weight — it is legible from across a desk precisely so that "this vault is open" is never a surprise. No copy anywhere may imply that Veil2 secures itself when the user walks away; Requirements §7 lists this among the things it does not do.

The locked state is a distinct screen, not a greyed-out list. Leaving the file list visible while locked would suggest the index is still readable, which is precisely the confusion HC-1 exists to prevent.

---

## 9. Design-Driven Requirements Feedback

Recorded per G-24. This section is a permanent record of what design work sent upstream, and it remains after absorption so the trace stays explicit. All three items were raised while the whole suite was unapproved and converging on a single version 1.0, so the owner absorbed them directly into Requirements v1.0 rather than deferring them to a later bump.

**1. Extraction is a fourth honesty moment. — Absorbed into FR-29.** FR-29 originally named three: unrecoverability at creation, the retained original after ingest, and deleted bytes persisting until compaction. Designing §6 made a fourth unavoidable — the moment a file is saved out of the vault and becomes an ordinary unprotected file. It is the most frequent of the four and the likeliest route by which data escapes Veil2. FR-29 now names it; §6 carries the wording.

**2. S-4 needs entry-level attribution. — Absorbed into S-4.** S-4 originally required only that damage be contained to the entries in the damaged region. §4.3 needs more than containment: to mark affected rows in the list rather than raising a whole-vault alarm, the core must report *which* entries a damaged region holds. S-4 now requires that attribution, since it is a capability and not a presentation choice.

**3. Locking on idle or system sleep was unspecified. — Absorbed into FR-3 and Requirements §7.** §8.5 covered explicit locking and locking on quit, but whether an unlocked vault should lock itself after inactivity, on screen lock, or on sleep was absent from the Requirements while carrying a direct security consequence. Raised here rather than decided, because it belonged to the Requirements owner. Resolved as explicit locking only: FR-3 now states the prohibition and its reason, Requirements §7 names the resulting exposure, and §8.5 above carries both into the design.

---

## 10. Open Questions

- **Whether grouping by folder is on or off by default.** It depends on whether real vaults are flat media dumps or structured imports. Resolver: tune with use, once real vaults exist.
- **Whether the CLI's user-facing strings are shared with the GUI's or maintained separately.** §7 requires identical vocabulary; whether that is enforced mechanically or by review is a build question. Resolver: Technical Specification.
- **Palette values in §2.2 against real content.** Chosen for contrast on paper, not yet checked against dense lists of long filenames in both themes. Resolver: tune with use.
- **Application icon and installer identity.** Not yet designed; §1.2 forbids padlocks and shields, which rules out most of the category's visual conventions and leaves the problem genuinely open. Resolver: Design Guideline, next version.
- **Whether search covers folder metadata as well as filenames**, and whether it is literal or fuzzy. Resolver: Design Guideline, next version, informed by the entry counts real vaults reach.

### Resolved during v1.0

- **Whether an unlocked vault locks itself on idle, screen lock, or system sleep.** Resolved as explicit locking only — the vault locks when the user locks it or quits, and on no other signal. The decision and its reason live in FR-3; the exposure it accepts is listed in Requirements §7; the design consequence and the prohibition on copy that would obscure it are in §8.5 above.
