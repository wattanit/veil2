# Veil2 — Requirements Document

**Version:** 1.1
**Status:** approved
**Date:** 2026-08-12
**Owner:** wattanit
**Companion documents:**
- Design Guideline v1.4 — downstream
- Technical Specification v1.0 — downstream

Legacy reference only, not a companion document: the original Veil project (`github.com/wattanit/veil`, 2025) and its `Requirements.md`. This document supersedes it entirely. Veil2 changes the central design decision (Veil stored its file index in plaintext) and the product form. No identifiers from that document carry into this one.

This document specifies what Veil2 must do and the acceptance criteria for each requirement. Visual design and interaction are specified in the Design Guideline. Implementation is specified in the Technical Specification. Where a mechanism is described below, the mechanism itself is the requirement, and the reason is stated with it.

---

## 1. Purpose and Motivation

Veil2 is a file-level encrypted vault: a container for files, encrypted individually. It is not a mounted virtual drive.

**1. Selective access to a large vault.** Retrieving one file must not require decrypting the entire vault. Veil2 encrypts each file individually rather than presenting a single decrypted volume. At vault sizes of hundreds of gigabytes, all-or-nothing decryption is impractical.

**2. Full control.** Only published, reviewed cryptography is used. The format is self-describing and can be inspected independently of the application. There is no recovery mechanism and no dependency outside this repository.

File names are encrypted along with file content, as a result of encrypting the index. This is not a separate requirement.

---

## 2. Scope

### 2.1 In scope

Creating, opening, and closing one vault at a time. Browsing the file index while a vault is open. Adding files and folders. Retrieving files to a user-chosen destination. Replacing and deleting entries. A graphical application and a command-line application, built on one shared core. Changing a vault's password.

**Platform scope: macOS only.** This version targets, builds for, and makes claims about macOS only. Windows and Linux are not scheduled. If either is built later, it is evaluated independently; compatibility with macOS-written vaults is not guaranteed in advance.

### 2.2 Explicitly deferred

Each entry states what must remain possible in a later version. A deferred item without a stated path forward is out of scope, not deferred.

- **Mounting a vault as a filesystem.** A mounted volume (as in VeraCrypt) would let other applications use vault contents directly, but requires a filesystem driver per platform. *Requirement to preserve:* the storage format must permit reading one entry's data without reading unrelated data (A-5).
- **Additional credentials (second password, key file, recovery key).** *Requirement to preserve:* the password must wrap a master key rather than derive content keys directly, so additional unwrap paths can be added without re-encrypting stored content (A-6).
- **Opening a vault file directly in an external application.** Requires writing plaintext to disk and cleaning it up afterward. A prior implementation of this feature deleted unrelated empty directories. *Requirement to preserve:* retrieval targets a caller-supplied destination (FR-17), so a supervised temporary-extraction path can be added without changing the retrieval model.
- **Editing a file in place and writing it back.** *Requirement to preserve:* replace-by-path (FR-13) already supports the storage side of this; no format change is needed for an editor integration.
- **More than one vault open at once.** *Requirement to preserve:* an open vault is represented as an instance with no process-global state (A-7).
- **Windows and Linux.** Not built, not scheduled. *Requirement to preserve:* HC-5 requires every vault to record its format version, so a later platform can be supported — including with a format change — without ambiguity about existing vault contents.
- **Migrating a vault from a superseded format version to the current one.** A later release may change key derivation or data layout; automatic migration of existing vaults is not implemented now. *Requirement to preserve:* every vault records its required format version and the writing application's version (HC-5); readers dispatch on the recorded format version rather than assuming the current one (FR-6). This allows a migration path to be added later without ambiguity about existing vault contents.

### 2.3 Out of scope

- **Multi-user access, sharing, and per-file permissions.** Requires identity and key management; a different product.
- **Reconciling divergent copies of a vault.** Veil2 detects an externally modified vault and refuses to overwrite it (FR-24). Resolving which of two diverged copies is authoritative is the responsibility of whatever tool created the divergence.
- **Concealing vault size, file count, or existence.** Defeating size and traffic analysis requires padding and fixed-size allocation. The storage and performance cost is not justified by the threat model in §7.
- **Hidden or deniable volumes.** The guarantee is unreliable under forensic examination, and implementation cost is high.
- **Securely erasing plaintext originals from the user's disk.** Veil2 does not control the storage medium; wear-levelling, snapshots, and backups can defeat file overwriting. This limitation is disclosed to the user (FR-27) rather than implying a guarantee that cannot be met.

---

## 3. Hard Constraints

These are defect-grade. A release that violates any of them is defective by definition.

**HC-1.** An unopened vault discloses no file name, path, size, timestamp, or content. What remains observable without the password: the vault's total size on disk, the number and sizes of its component files, and the fact that it is a Veil2 vault.

**HC-2.** No operation writes plaintext of vault contents to any location the user has not designated for that operation. This includes temporary files, caches, logs, and crash artifacts.

**HC-3.** Any alteration of stored data — modification, truncation, reordering, or substitution of one file's data for another's — is detected before that data is presented to the user, and fails the operation with an error naming what failed. Partial, truncated, or unverified output is never reported as success.

**HC-4.** No single interruption — crash, power loss, media removal, or user cancellation — leaves a vault that cannot be opened, and none destroys the only copy of data the vault held before the operation began.

**HC-5.** A vault is self-describing. It records the format version required to read it, every parameter needed to derive its keys, and the version of the application that wrote it. The format version gates whether a reader can open it; the application version is provenance only. Many application versions may write one format version, and changing a default parameter in a later release never renders an existing vault unopenable.

**HC-6.** Only published, widely reviewed cryptographic primitives and constructions are used. No custom primitive, no custom mode of operation, no custom key-derivation scheme.

**HC-7.** Loss of the vault password is unrecoverable. No escrow, no reset, no recovery path — stated before the vault is created.

---

## 4. Functional Requirements

Requirement numbers are continuous across subsections. Subsection headings are for organization only and are not part of the identifier.

### 4.1 Vault lifecycle

**FR-1.** Create a vault at a user-chosen location, with the password set at creation and subject to the minimum in C-4.

**FR-2.** Open a vault by password. A wrong password and a damaged vault are reported as distinct conditions. The original Veil reported both identically as a cryptography error, which misdirects users to the wrong remedy.

**FR-3.** Lock a vault on user instruction and on application exit. Locking releases derived key material from memory, clears any preview content held in memory (FR-30), and releases the lock described in FR-23. The vault is not locked automatically on inactivity, sleep, or screen lock. This exposure is disclosed in §7 and must be disclosed in the product.

**FR-4.** Change a vault's password. Completion time is independent of vault size; stored content is not re-encrypted.

**FR-5.** Refuse to open a vault whose format version is newer than the release supports. Report the required version. Guessing at an unrecognized format risks violating HC-3.

**FR-6.** Identify a vault written in an older format version and report the version. Open it if the release still supports that version. If support has been withdrawn, report both the vault's format version and the last release able to read it.

### 4.2 Browsing

**FR-7.** On open, present the complete index: name, relative path, size, and timestamps for every entry, without decrypting any file's content. Open time is proportional to entry count, independent of total vault size.

**FR-8.** Group and filter the index by each entry's recorded relative path. Storage is flat; path is metadata, not structure. Operations implying a real directory tree — renaming a folder, creating an empty one — are not supported.

**FR-28.** On request, present an entry's complete recorded metadata — name, folder, size, source modification time, and added time — beyond what the list columns (FR-7) show. Nothing here requires decrypting content.

**FR-29.** Group and filter the index by each entry's file extension: the substring of `name` following its last `.`, excluded when that `.` is the first character of the name — so `.gitignore` has no extension, and `archive.tar.gz` groups under `gz`, not `tar.gz`. An entry with no extension groups under one reserved bucket. Comparison is case-insensitive. This is a second flat view control alongside FR-8's folder grouping, under the same restriction: no rename, create, or drag is implied by a group, extension or folder.

**FR-30.** For a single selected entry whose extension is on the supported preview list (images: `jpg`, `jpeg`, `png`, `gif`, `webp`, `bmp`; text: `txt`, `md`, `log`, `csv`, `json`) and no larger than C-5, decrypt its content to memory only — never to a temporary file — and display it. Markdown and other text types are shown as plain text, not rendered: the interface's Content-Security-Policy (Technical Specification §5.3) forbids network requests at runtime, and rendering vault content as HTML would put untrusted bytes in a position to attempt one anyway. FR-18's verification runs before anything is displayed; a failed check shows the same failure extraction shows, and nothing is displayed. Preview content is cleared — not merely dereferenced — no later than when the preview closes, the vault locks (FR-3), or the application exits.

This introduces no new extraction path: it is FR-17 and FR-18's existing guarantee with the destination held in memory and displayed rather than written to a chosen file. No CLI equivalent is owed under A-4 — a terminal has no display surface to preview onto, and the underlying capability (save a copy) already has one there. Preview is a narrower carve-out from the "not a file manager" anti-goal (Design Guideline §1.2), which the Design Guideline must state explicitly rather than leave in silent contradiction; rename-in-place, folder creation, and move-between-folders remain excluded, since those still imply a directory tree the storage format does not have.

### 4.3 Ingest

**FR-9.** Add one or more files. Source files are read and left unmodified; ingest never deletes the original. An interrupted or failed ingest cannot lose data. The retained, unprotected copy is disclosed to the user (FR-27).

**FR-10.** Add a folder. Every regular file beneath it is stored, each recording its path relative to the added root's *parent* — that is, the added root's own name is preserved as the top-level folder segment, not discarded. Without it, a file directly inside one added folder and a same-named file directly inside a *different* added folder would both land at the same path and collide as one identity (FR-13, FR-14) despite being two distinct files (FR-8).

**FR-11.** Do not follow symbolic links during folder ingest. Record each as skipped. Following links risks cycles and can capture data outside the selected tree.

**FR-12.** Report ingest as successful only after stored data is durable. A power loss immediately after the success report must not lose the entry.

**FR-13.** Replace an existing entry, matched on full path (folder and name together). `work/2024/report.pdf` is replaced only by an entry at that exact path. New content is durable before previous content becomes unreachable; an interruption leaves exactly one intact version (HC-4).

**FR-14.** Refuse to add a file at a path the vault already holds. Report the conflicting path and reference replacement as the alternative. FR-13 defines full path as identity; two entries at one path would make later operations on that path ambiguous.

**FR-15.** Report progress and accept cancellation for every ingest. A cancelled ingest leaves the vault as though it had not been started.

**FR-16.** Reject an addition that would exceed the per-file size limit (C-2) or the per-vault entry limit (C-1). Report the limit and the value that would result.

### 4.4 Retrieval

**FR-17.** Extract selected entries to a destination the user supplies. Extraction is the only path by which plaintext leaves a vault (HC-2).

**FR-18.** Verify integrity during extraction. On failure, the operation errors, and incomplete output is removed or marked unusable. Incomplete output is never left appearing valid (HC-3).

**FR-19.** Never overwrite an existing file at the destination without explicit confirmation identifying the file. (The original Veil overwrote destination files without confirmation.)

**FR-20.** Report progress and accept cancellation for every extraction.

**FR-21.** Extract with memory use independent of the file's size, so that files at the C-2 limit are retrievable on ordinary hardware.

### 4.5 Removal

**FR-22.** Delete an entry, making it immediately unreachable and removing it from the index.

### 4.6 Concurrent and external modification

**FR-23.** Take an advisory lock when opening a vault, and tell a second opener that the vault is in use rather than allowing two writers.

If storage does not support locking — read-only media, a filesystem without lock support, a directory the user cannot write to — the vault opens read-only and reports this at open time. Read operations (browsing, retrieval, verification) function normally. Only actual lock contention is reported as in-use.

Two specific behaviors are excluded, for the reasons stated. A read-only vault must not be refused at open: refusing it would turn an interrupted operation on media that later became write-protected into permanent data loss (HC-4), and would prevent the one operation (FR-26) that could diagnose a failing drive. Unavailable locking must not be reported as contention: this would misdirect the user to look for a second process that does not exist (see FR-2).

**FR-24.** Detect that a vault has changed on disk since it was opened. Refuse to write over the change. Offer to reload. An external writer is treated as an expected condition.

**FR-25.** Handle the storage medium disappearing mid-operation without crashing or requiring a restart. The operation fails cleanly and the application stays usable afterward.

---

### 4.7 Verification

**FR-26.** Verify a whole vault on the user's instruction: read and authenticate all stored data, compare every entry against its recorded content hash, and report by name each entry that fails. Nothing is extracted and nothing is modified.

This is the only mechanism for detecting decayed stored data before it is needed. HC-3 detects damage only when data is read (see §7); without verification, damage is discovered only when a file is retrieved. Verification reads the entire vault; it is user-initiated, cancellable, and never automatic or scheduled.

Verification detects damage; it does not repair it. Veil2 stores no redundancy. A failed verification identifies files that are already lost, which is useful for locating backups but is not a recovery mechanism. This limitation is disclosed alongside the result.

---

## 5. Architecture Requirements

**A-1.** The core is a library with no interactive input or output, and no dependency on a terminal or graphical shell. Credentials are passed as parameters; the core does not prompt for input. (The original Veil coupled its logic to CLI password prompts, resulting in fourteen unit tests, no integration tests, and no way to exercise it without a pseudo-terminal.)

**A-2.** Every operation over a file's data is streaming. The core never requires a whole file to be resident in memory (S-1).

**A-3.** Every long-running operation exposes progress reporting and cooperative cancellation, as required by FR-15 and FR-20.

**A-4.** The graphical and command-line applications are peer consumers of the core and contain presentation logic only. Every capability available in one is available in the other. The command-line application also serves as the integration-test surface for the core.

**A-5.** The storage format permits reading one entry's data without reading unrelated entries' data, as required by FR-7 and FR-17 (see also §2.2).

**A-6.** The password protects a master key, which protects stored content; the password does not protect content directly, as required by FR-4 (see also §2.2).

**A-7.** An open vault is represented as an instance with no process-global state. The single-vault-at-a-time limit (§2.1) is a product decision, not a structural one (§2.2).

---

## 6. Configuration and Stability Requirements

### 6.1 Configuration

Values below are initial; tune with use.

**C-1.** Maximum entries per vault: 65,536, chosen to exceed the target workload while keeping the index small enough to rewrite atomically.

**C-2.** Maximum size of a single stored file: 64 GiB.

**C-3.** Key-derivation cost parameters are stored per vault (HC-5), chosen so that opening a vault takes approximately one second on contemporary desktop hardware.

**C-4.** Minimum password length: 12 characters. (The original Veil required 8.)

**C-5.** Maximum size of an entry eligible for in-app preview (FR-30): 50 MiB. Decrypting a preview into memory costs nothing at this size; entries above it remain retrievable via FR-17, just not previewed inline.

### 6.2 Stability and quality

**S-1.** Peak memory use during any operation does not grow with the size of the file being processed or the size of the vault.

**S-2.** Vault open time is proportional to entry count, not to vault size (restates the acceptance standard of FR-7 as a standing property).

**S-3.** Damage to a region of stored data renders unreadable only the entries stored in that region. The index and all other entries remain retrievable. Affected entries can be individually identified and reported, so a partial failure is presented as a list of unreadable files, not a failure of the vault.

---

## 7. Threat Model and Non-Guarantees

This section defines what Veil2 claims to protect against. FR-27 requires these limits to be disclosed in the product where a user could otherwise assume broader protection.

**Threat model.** Veil2 defends against an adversary who obtains the vault at rest (a stolen device or drive, a copied file, a discarded disk, a backup) and does not have the password. HC-1 and HC-3 are the guarantee against this adversary.

**What Veil2 does not defend against, and does not claim to:**

- **A compromised host.** Keyloggers, screen capture, memory inspection of an unlocked vault, and malicious processes reading extracted files are not defended against. Veil2 protects data at rest, not a compromised machine.
- **Two machines writing one vault at once.** Locking is advisory and can fail to prevent a second writer. Veil2 detects the resulting conflict and refuses to write over it (FR-24); this is detection, not prevention.
- **An unattended machine with a vault unlocked.** Veil2 locks only on explicit user action or application exit (FR-3); it does not lock on inactivity, sleep, or screen lock. An unlocked vault on an accessible machine is readable by anyone with access. The operating system's screen lock, not Veil2, is the protection in this case.
- **Anything already extracted.** A file saved out of a vault is an ordinary file with ordinary permissions. Veil2's responsibility ends at the destination the user chose.
- **Originals left behind.** Ingest copies rather than moves (FR-9), so the unprotected original remains until the user removes it.
- **Volume and timing observation.** An adversary who can observe the vault's storage over time learns approximately how much data was added and when. Concealing this is out of scope (§2.3).
- **Coercion.** There are no hidden volumes and no deniability. A user compelled to give up the password gives up the vault.
- **Continuous tamper detection.** Modification is detected when the affected data is read (HC-3), not when the vault is opened. An entry's integrity is unknown until it is retrieved or verified (FR-26). Veil2 does not monitor a vault in the background; a verification result describes the vault's state at the time it ran.
- **Password loss.** HC-7. There is no recovery, by design.

**FR-27.** Disclose the following limits in the product, not only in documentation: unrecoverability, at vault creation; the retained original, after ingest; and the unprotected status of an extracted file, at extraction. Extraction disclosure addresses the most common route by which data leaves Veil2's protection. Wording is specified in the Design Guideline; the disclosure itself is required here.

---

## 8. Deliverables and Document Plan

Three foundation documents, written in order — Requirements, then Design Guideline, then Technical Specification — because design constrains structure more than structure constrains design, and because the Spec must cite both.

- **This document** owns what Veil2 must do and why.
- **Design Guideline** owns identity and anti-goals, the single-panel layout, drag-and-drop affordances, how progress and cancellation are presented, how failure and constrained conditions are communicated, the wording of the honesty clauses FR-27 requires, and the unlock moment.
- **Technical Specification** owns the container format, the cryptographic construction, crate and workspace structure, dependencies, testing strategy, and milestones. Every choice in it that satisfies a requirement here cites that requirement's identifier.

Downstream of the suite: an Implementation Plan expanding the Spec's milestones, per-phase task lists, and test cases. Each test case cites the requirement it verifies.

Document versions and release versions are independent counters. Foundation documents in this suite begin at 1.0. Released software begins at 2.0.0, continuing the original Veil's lineage rather than restarting, so that a version number never refers to two different products. Each release's Implementation Plan pins the exact foundation document versions it was built against; that pin, not these documents, is the as-built record of what a release shipped.

**A release names the platforms it was run on.** 2.0.0 is macOS (§2.1). Windows and Linux follow as their own releases once each has been built and run there, and neither is announced as supported before that. "Coming soon" is the honest statement and the only one this document permits; a download button for a binary nobody has executed is not.

---

## 9. Open Questions

- **Exact key-derivation cost parameters satisfying C-3.** Measured at 0.27s on the development machine, which is high-end relative to the range C-3 targets. Not yet measured on low-end hardware. A later change does not orphan existing vaults (HC-5). Resolver: Technical Specification.
- **Maximum length of the path metadata recorded under FR-10.** Resolver: Technical Specification.
