# Veil2 — Requirements Document

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-08
**Owner:** wattanit
**Companion documents:**
- Design Guideline v1.0 — downstream
- Technical Specification v1.0 — downstream

Prior art, not a companion: the original Veil project (`github.com/wattanit/veil`, 2025) and its `Requirements.md`. That document is superseded wholesale rather than amended — Veil2 reverses its central decision (a deliberately plaintext file index) and changes the product form. It carries no identifiers this document continues.

This document owns **what Veil2 must do and why it exists**. It states behavior and the acceptance standard for that behavior. It defers how the product looks, feels, and speaks to the Design Guideline, and how it is built to the Technical Specification. Where a mechanism appears below, it appears because the mechanism is itself the requirement, and the reason is stated in the same breath.

---

## 1. Purpose and Motivation

In priority order:

**1. Selective access without whole-vault decryption.** A user holding a several-hundred-gigabyte vault must be able to retrieve one file from it without decrypting anything else. This is the reason Veil2 encrypts per file rather than presenting a decrypted volume: volume encryption makes access an all-or-nothing act, and at this scale all-or-nothing is unusable.

**2. Names are secrets.** A vault that protects file contents while publishing file names protects very little — `/HR/salaries/exec_compensation_2024.csv` discloses the sensitive fact without decrypting a byte. The original Veil stored its index in plaintext and this was its defining flaw; Veil2 exists in large part to correct it.

**3. Storage location is not a trust decision.** A vault must be safe to place on cloud-sync folders, external drives, and removable media without changing what it protects or what it discloses. Users should not have to reason about where a vault may live.

**4. Failure is loud.** Corruption, truncation, interruption, and media loss must produce errors, never silently degraded output. The original Veil returned a truncated file, reported success, and exited zero; Veil2 treats that class of behavior as a defect rather than a limitation.

---

## 2. Scope

### 2.1 In scope

Creating, opening, and closing a single vault at a time. Browsing the complete file index while a vault is open. Adding files and folders. Retrieving selected files to a user-chosen destination. Replacing and deleting entries. Explicit, user-initiated compaction. A graphical application and a command-line application, both built on one shared core. Changing a vault's password. All of it on macOS, Windows, and Linux, as peer platforms rather than a primary and two ports.

### 2.2 Explicitly deferred

Each entry names the door the current version must leave open. A deferral without a door is an out-of-scope item wearing optimism.

- **Mounting a vault as a filesystem.** The VeraCrypt-style mount is the natural long-term answer to using vault files in other applications, but it demands platform-specific filesystem drivers on every target. *Door:* the storage format must permit reading one file's data without reading unrelated data (A-5).
- **Additional credentials — second password, key file, recovery key.** *Door:* the password must wrap a master key rather than derive content keys directly, so further unwrap paths can be added without re-encrypting stored content (A-6).
- **Opening a vault file directly in an external application.** This requires plaintext on disk and therefore a cleanup obligation; the original Veil's attempt at this produced a command that deleted unrelated empty directories. *Door:* retrieval targets a caller-supplied destination (FR-16), so a supervised temporary-extraction path can be added without changing the retrieval model.
- **Editing a file in place and writing it back.** *Door:* replace-by-name (FR-13) already expresses the storage half of this, so an editor integration needs no new format support.
- **More than one vault open at once.** *Door:* the core must represent an open vault as an instance value with no process-global state (A-7).
- **Migrating a vault from a superseded format version to the current one.** A later release may change how keys are derived or how data is laid out; rewriting existing vaults into the new format is not built now. *Door:* every vault records the format version required to read it and the application version that wrote it (HC-5), and readers dispatch on the recorded format version rather than assuming the current one (FR-30) — so a migration path can be added later without having to guess what an existing vault contains.

### 2.3 Out of scope

- **Multi-user access, sharing, and per-file permissions.** These require identity and key management, which is a different product.
- **Reconciling divergent copies of a vault.** Veil2 detects that a vault changed underneath it and refuses to corrupt it (FR-27); deciding which of two diverged copies wins belongs to the sync tool that created the divergence.
- **Concealing vault size, file count, or existence.** Defeating size and traffic analysis requires padding and fixed-size allocation whose storage and performance cost is not justified against the threat model in §7.
- **Hidden or deniable volumes.** The guarantee is fragile under realistic forensic examination and the implementation cost is large.
- **Securely erasing plaintext originals from the user's disk.** Veil2 cannot make honest guarantees about media it does not control — wear-levelling, snapshots, and backups all defeat naive overwriting. Users are told this rather than sold a false assurance (FR-29).

---

## 3. Hard Constraints

These are defect-grade. A release that violates any of them is defective by definition.

**HC-1.** An unopened vault discloses no file name, path, size, timestamp, or content. What remains observable without the password, and is accepted: the vault's total size on disk, the number and sizes of its component files, and the fact that it is a Veil vault.

**HC-2.** No operation writes plaintext of vault contents to any location the user has not designated for that operation. This includes temporary files, caches, logs, and crash artifacts.

**HC-3.** Any alteration of stored data — modification, truncation, reordering, or substitution of one file's data for another's — is detected before that data is presented to the user, and fails the operation with an error naming what failed. Partial, truncated, or unverified output is never reported as success.

**HC-4.** No single interruption — crash, power loss, media removal, or user cancellation — leaves a vault that cannot be opened, and none destroys the only copy of data the vault held before the operation began.

**HC-5.** A vault is self-describing. It records the format version required to read it, every parameter needed to derive its keys, and the version of the application that wrote it. Format version and application version are separate fields with separate lifecycles: the format version is the compatibility gate a reader must satisfy, while the writing application's version is provenance and never gates access. Many application versions may write one format version. Changing a default parameter in a later release never renders an existing vault unopenable.

**HC-6.** Only published, widely reviewed cryptographic primitives and constructions are used. No custom primitive, no custom mode of operation, no custom key-derivation scheme.

**HC-7.** Loss of the vault password is unrecoverable, and the product states this before the vault is created. There is no escrow, no reset, and no recovery path. This is a product decision, not an implementation gap.

**HC-8.** A vault is portable across every supported platform. A vault written on one opens on the others with identical contents and identical file names, without conversion. Platform-specific behaviour — path separators, Unicode normalisation of names, case sensitivity, reserved characters — is normalised on the way in and never reaches the stored format. A vault that opens on the platform that wrote it and fails on another is defective. Users move vaults between machines on drives and sync services (§1, motivation 3), so portability is a property of the format, not a convenience.

---

## 4. Functional Requirements

Numbering is continuous across groups; the headings are organisation, not namespaces.

### 4.1 Vault lifecycle

**FR-1.** Create a vault at a user-chosen location, with the password set at creation and subject to the minimum in C-4.

**FR-2.** Open a vault by password. A wrong password is reported as a wrong password, distinguishably from a damaged vault — conflating the two, as the original Veil did by surfacing every failure as a cryptography error, sends users to the wrong remedy.

**FR-3.** Lock a vault on the user's instruction and on application exit, releasing derived key material from memory and releasing the lock of FR-26. Locking is explicit: Veil2 does not lock on inactivity, on system sleep, or when the screen locks. This is a decision rather than an omission — a vault that re-prompts partway through a working session teaches people to retype their password without reading the screen, which is its own hazard. The exposure it leaves is stated in §7 and must be stated in the product.

**FR-4.** Change a vault's password. Acceptance: completion time is independent of the vault's size, since content is not re-encrypted.

**FR-5.** Refuse to open a vault whose format version is newer than the release understands, naming the version required. Refusing is the correct behavior; guessing at an unknown format risks HC-3.

**FR-30.** Identify a vault written in a format version older than the current one, report which version it uses, and open it where the release still supports that version. Where support has been withdrawn, name both the vault's format version and the last release able to read it. A superseded format is a known condition to be handled, not a failure — refusing it outright would strand data Veil2 itself wrote.

### 4.2 Browsing

**FR-6.** On open, present the complete index — name, original relative path, size, and timestamps for every entry — without reading or decrypting any file's content. Acceptance: open time is proportional to entry count and independent of total vault size.

**FR-7.** Group and filter the index by each entry's recorded original relative path. Storage is flat; path is descriptive metadata, so operations that a real tree would support — renaming a folder, creating an empty one — are not offered, and the product does not imply otherwise.

**FR-8.** Report the statistics a user needs to decide whether compaction is worth running: entry count, logical bytes stored, physical bytes occupied on disk, bytes reclaimable by compaction, and reclaimable bytes as a share of physical. Compaction is a deliberate manual act (FR-23), so the figures the decision rests on must be in front of the person making it.

### 4.3 Ingest

**FR-9.** Add one or more files. The source file is read and left unmodified; Veil2 never deletes a user's original as part of ingest. An interrupted or failed ingest therefore cannot lose data. The cost — an unprotected copy remains on disk — is stated to the user rather than silently accepted (FR-29).

**FR-10.** Add a folder, storing every regular file beneath it, each recording its path relative to the added root as the metadata of FR-7.

**FR-11.** Do not follow symbolic links during folder ingest; record each as skipped. Following them risks cycles and captures data outside the tree the user selected.

**FR-12.** Report an ingest as successful only once the stored data is durable. Acceptance: an immediate power loss after the success report does not lose the entry.

**FR-13.** Replace an existing entry by name. The new content is durable before the previous content becomes unreachable, so an interruption leaves one intact version and never zero (HC-4).

**FR-14.** Report progress and accept cancellation for every ingest. A cancelled ingest leaves the vault as though it had not been started.

**FR-15.** Reject an addition that would exceed the per-file limit (C-2) or the per-vault entry limit (C-1), naming the limit and the current value in the message.

### 4.4 Retrieval

**FR-16.** Extract selected entries to a destination the user supplies. Extraction is the only path by which plaintext leaves a vault (HC-2).

**FR-17.** Verify integrity during extraction. On failure, the operation errors and the incomplete output is removed or clearly marked as unusable — never left in place looking like a valid file (HC-3).

**FR-18.** Never overwrite an existing file at the destination without explicit confirmation naming the file. The original Veil overwrote silently, and a failed extraction destroyed the user's only good copy.

**FR-19.** Report progress and accept cancellation for every extraction.

**FR-31.** Where a stored name cannot be represented on the platform being extracted to — a reserved character or device name, or a collision on a case-insensitive filesystem — stop and ask for an alternative rather than altering the name silently. HC-8 makes the vault's names authoritative, and a file written under a quietly different name no longer matches what the vault reports it holds.

**FR-20.** Extract with memory use independent of the file's size, so that files at the C-2 limit are retrievable on ordinary hardware.

### 4.5 Removal and compaction

**FR-21.** Delete an entry, making it immediately unreachable and removing it from the index. Its stored bytes may remain in the vault until compaction, and the product must say so wherever deletion is offered — a user who deletes a file and then hands the vault to someone else must not believe those bytes are gone (FR-29).

**FR-22.** Make the statistics of FR-8 available immediately on opening a vault, without reading or scanning stored data. Acceptance: the figures appear at open time for a vault of any size. Deriving reclaimable space by scanning hundreds of gigabytes would cost more than the compaction it is meant to advise, so these totals are maintained as the vault changes rather than computed on demand.

**FR-23.** Compact only when the user explicitly asks. Compaction rewrites stored data and is never triggered automatically or in the background, where it would compete for I/O and risk interruption the user did not choose.

**FR-24.** Perform compaction such that the vault is openable at every point during it, and an interruption costs at most the progress of the current unit of work (HC-4).

**FR-25.** Bound compaction's working space requirement independently of total vault size. Acceptance: compacting a 500 GB vault does not require anything approaching 500 GB of free space. This requirement is what makes compaction possible at the sizes in §1; a format requiring whole-vault rewrite fails it.

### 4.6 Concurrent and external modification

**FR-26.** Take an advisory lock when opening a vault, and tell a second opener that the vault is in use rather than allowing two writers. On storage where advisory locking is unreliable — network filesystems and some user-space mounts — the lock is best-effort and FR-27's detection is the actual protection; the product says so rather than implying a guarantee it cannot keep.

**FR-32.** Reconcile stored data against the index when opening a vault, discarding stored data that no index entry references — the residue an interrupted ingest or compaction leaves behind under HC-4. Report the space recovered rather than absorbing it silently. Where the vault cannot be written, open it read-only and say so instead of failing; a vault on read-only media must still be readable.

**FR-27.** Detect that a vault changed on disk since it was opened, refuse to write over the change, and offer to reload. Vaults may live in sync folders (§1, motivation 3), so an external writer is an expected condition, not an anomaly.

**FR-28.** Handle the storage medium becoming unavailable mid-operation by failing that operation within HC-4, without crashing and without leaving the application in a state that requires restarting it.

---

## 5. Architecture Requirements

**A-1.** The core is a library with no interactive input or output and no assumption of a terminal or a graphical shell. Credentials are passed to it as parameters; it never prompts. The original Veil placed its logic in the CLI layer coupled to password prompts, which is why it has fourteen unit tests, no integration tests, and cannot be exercised without a pseudo-terminal.

**A-2.** Every operation over a file's data is streaming. The core never requires a whole file to be resident in memory (S-1).

**A-3.** Every operation that can run long exposes progress reporting and cooperative cancellation. Both are required by FR-14 and FR-19 in the graphical application, and retrofitting either into a completed core is expensive.

**A-4.** The graphical and command-line applications are peer consumers of the core, holding presentation logic only. Every capability available in one is available in the other. This keeps the command-line application a genuine deliverable and makes it the integration-test surface for the core.

**A-5.** The storage format permits reading one entry's data without reading unrelated entries' data. Required by FR-6 and FR-16, and the door held open for the mount deferral in §2.2.

**A-6.** The password protects a master key which in turn protects stored content, rather than protecting content directly. Required by FR-4, and the door held open for additional credentials in §2.2.

**A-7.** An open vault is represented as an instance with no process-global state, so that the single-vault limit is a product decision rather than a structural one (§2.2).

---

## 6. Configuration and Stability Requirements

### 6.1 Configuration

Values below are initial; tune with use.

**C-1.** Maximum entries per vault: 65,536. Chosen well above the media-library workload Veil2 targets while keeping the index small enough to rewrite atomically on every change.

**C-2.** Maximum size of a single stored file: 64 GiB.

**C-3.** Key-derivation cost parameters are stored per vault (HC-5) and chosen so that opening a vault takes roughly one second on contemporary desktop hardware — enough to make guessing expensive, little enough that unlocking does not feel broken.

**C-4.** Minimum password length: 12 characters. Higher than the original Veil's 8, because HC-7 makes the password the only thing standing between the user and total loss.

### 6.2 Stability and quality

**S-1.** Peak memory use during any operation does not grow with the size of the file being processed or the size of the vault.

**S-2.** Vault open time is proportional to entry count, not to vault size (restates the acceptance standard of FR-6 as a standing property).

**S-3.** Adding, replacing, or deleting an entry dirties on-disk bytes proportional to the size of that change, not to the size of the vault. Acceptance: adding a 2 MB file to a 300 GB vault causes an incremental backup or file-sync client to transfer on the order of megabytes. Without this, the location independence of §1, motivation 3 is unachievable — a vault held in one monolithic file re-uploads in full on every change.

**S-4.** Damage to a region of a vault's stored data renders unreadable only the entries stored in that region; the index and all other entries survive and remain retrievable. Damage is further attributable: the entries affected by a damaged region can be identified and reported individually, so that a partial failure is presented as a list of unreadable files rather than as a failure of the vault. At the sizes in §1, media errors are an expected event over a vault's lifetime, and a format in which one bad sector loses everything — or in which one bad sector cannot be told apart from total loss — is precisely the failure this project rejects.

---

## 7. Threat Model and Non-Guarantees

This section is itself a requirement: it fixes what Veil2 claims, and FR-29 obliges the product to repeat these limits where a user could otherwise over-trust it.

**What Veil2 defends against:** an adversary who obtains the vault at rest — a stolen laptop or drive, a synced copy in someone else's cloud account, a discarded disk, a backup tape — and who does not have the password. Against that adversary, HC-1 and HC-3 are the guarantee.

**What Veil2 does not defend against, and does not claim to:**

- **A compromised host.** Keyloggers, screen capture, memory inspection while a vault is unlocked, and malicious processes reading extracted files all defeat Veil2 completely. It protects data at rest, not a machine already under someone else's control.
- **Two machines writing one vault over a network filesystem.** Advisory locking is unreliable on NFS, SMB, and some user-space mounts (FR-26), so Veil2 cannot always prevent a second writer — it detects the resulting conflict and refuses to compound it (FR-27). Detection is not prevention, and a vault shared live between machines is outside what this design protects.
- **An unattended machine with a vault unlocked.** Veil2 locks only when the user locks it or quits the application (FR-3). It does not lock on inactivity, sleep, or screen lock, so an unlocked vault on a machine someone else can reach is readable by them. The operating system's own screen lock is the protection at that moment, and Veil2 does not substitute for it.
- **Anything already extracted.** A file saved out of a vault is an ordinary file with ordinary permissions. Veil2's responsibility ends at the destination the user chose.
- **Originals left behind.** Ingest copies rather than moves (FR-9), so the unprotected original remains until the user removes it.
- **Volume and timing observation.** An adversary who watches a vault in a sync folder over time learns approximately how much data was added and when. Concealing this is out of scope (§2.3).
- **Coercion.** There are no hidden volumes and no deniability. A user compelled to give up the password gives up the vault.
- **Continuous tamper detection.** Modification is detected when the affected data is read (HC-3), not at open. An untouched entry in a tampered vault is not known to be intact until it is retrieved.
- **Password loss.** HC-7. There is no recovery, by design.

**FR-29.** Surface these limits at the moments they matter rather than only in documentation: unrecoverability at vault creation, the retained original after ingest, the persistence of deleted bytes until compaction, and the unprotected status of any file saved out of a vault. The last is the most frequent of the four and the likeliest route by which data leaves Veil2's protection — a user who extracts a file and forgets it is now ordinary is the failure this requirement exists to prevent. Wording belongs to the Design Guideline; that it must be said is a requirement here.

---

## 8. Deliverables and Document Plan

Three foundation documents, written in order — Requirements, then Design Guideline, then Technical Specification — because design constrains structure more than structure constrains design, and because the Spec must cite both.

- **This document** owns what Veil2 must do and why.
- **Design Guideline** owns identity and anti-goals, the single-panel layout, drag-and-drop affordances, how progress and cancellation are presented, how failure and constrained conditions are communicated, the wording of the honesty clauses FR-29 requires, and the unlock moment.
- **Technical Specification** owns the container format, the cryptographic construction, crate and workspace structure, dependencies, testing strategy, and milestones. Every choice in it that satisfies a requirement here cites that requirement's identifier.

Downstream of the suite: an Implementation Plan expanding the Spec's milestones, per-phase task lists, and test cases. Each test case cites the requirement it verifies.

Document versions and release versions are independent counters. Foundation documents in this suite begin at 1.0. Released software begins at 2.0.0, continuing the original Veil's lineage rather than restarting, so that a version number never refers to two different products. Each release's Implementation Plan pins the exact foundation document versions it was built against; that pin, not these documents, is the as-built record of what a release shipped.

---

## 9. Open Questions

- **Exact key-derivation cost parameters satisfying C-3.** Resolver: Technical Specification, measured on real hardware.
- **Maximum length of the path metadata recorded under FR-10.** Resolver: Technical Specification.
- **Whether replace-by-name (FR-13) matches on file name alone or on name together with the recorded path metadata of FR-7.** Resolver: Design Guideline.
- **Whether the product offers a whole-vault verification operation** — reading and checking every entry without extracting — given that HC-3 is per-operation and §7 names this as a non-guarantee. Resolver: Design Guideline.
- **Whether the command-line application exposes compaction as a schedulable operation**, which would sit awkwardly with FR-23's prohibition on automatic compaction. Resolver: Design Guideline.
### Resolved during v1.0

Answers live in the documents named; these entries remain so the trace does not evaporate.

- **Which normalisation HC-8 mandates for stored file names**, and how FR-13 matches an existing entry. Resolved as Unicode NFC with exact, case-sensitive comparison — Technical Specification §4.6.
- **How long a superseded format version stays readable before support is withdrawn under FR-30.** Resolved as: withdrawal is not permitted while the migration path deferred in §2.2 remains unbuilt, because there would be no other route by which the user's data could be recovered — Technical Specification §11.1.
- **Behaviour when the index is intact but a component of the stored data is missing entirely.** Resolved as total damage to that component: the vault opens, the affected entries are enumerated and reported unreadable, and the rest stays retrievable — Technical Specification §4.5.
