# Veil2 — Technical Specification

**Version:** 1.1
**Status:** approved
**Date:** 2026-08-12
**Owner:** wattanit
**Companion documents:**
- Requirements Document v1.1 — upstream
- Design Guideline v1.3 — upstream

This document owns how Veil2 is built: system structure, execution model, data formats, the cryptographic construction, dependencies, build and release, testing, and milestones. It defers what the product must do to the Requirements Document and how it presents itself to the Design Guideline. Every choice that satisfies an upstream requirement cites that requirement's identifier. Values given as defaults are initial and tunable unless a requirement fixes them.

---

## 1. System Structure

A single Cargo workspace. The core is a library with no interactive input or output and no UI assumptions (A-1); the two applications are peer consumers holding presentation logic only (A-4).

```
veil2/
├── Cargo.toml              # workspace manifest, shared dependency versions
├── crates/
│   ├── veil-core/          # library: format, crypto, storage, index, vault API
│   ├── veil-cli/           # binary `veil`
│   └── veil-gui/           # desktop application, Tauri v2 (§5.3)
└── docs/                   # this suite
```

`veil-core` internal modules:

| Module | Owns |
|---|---|
| `crypto` | Key derivation, key hierarchy, streaming AEAD, zeroisation |
| `format` | On-disk layout, header, serialisation, version dispatch |
| `store` | Entry file read and write (§4.5) |
| `index` | Entry model, atomic index persistence, statistics |
| `vault` | Public API, orchestration, locking, progress and cancellation |
| `error` | Typed error taxonomy |

`crypto` is **not** split into a separate crate initially. Splitting it later is a mechanical move; splitting it now creates cross-crate churn while the format is still moving. It is kept free of dependencies on the other modules so the split stays cheap if independent audit becomes worthwhile.

---

## 2. Execution Model

**`veil-core` is synchronous and blocking. No async runtime.** Every operation is file-I/O bound, and a blocking API is callable from any caller — the CLI drives it directly, the GUI drives it on a worker thread. Requiring an async runtime would impose a large dependency and a colouring constraint on consumers for no benefit at this workload.

**Long operations take a progress sink and a cancellation token** (A-3). Cancellation is cooperative and checked at chunk boundaries, which bounds cancellation latency to one chunk of I/O. Both are parameters rather than global state, so the CLI can pass no-ops and the GUI can marshal to its UI thread.

**An open vault is a value, not a singleton** (A-7). `Vault` is `Send` and not `Sync`: one writer at a time within a process, which matches the single-writer guarantee the on-disk format assumes. Supporting several open vaults later is a caller-side change only.

**Advisory locking** (FR-23) uses an OS advisory lock held on `veil.lock` inside the vault directory (§4.1) for the lifetime of the open vault. Advisory locks are unreliable on some network filesystems and FUSE-backed mounts; on those, the lock is best-effort and the generation counter of §4.3 is the actual protection — Veil2 detects a conflicting write and refuses it rather than preventing it (FR-24).

**Failing to take the lock and being refused the lock are different conditions** (FR-23). Contention — another process holds it — is `VaultInUse`. Being unable to take one at all, because the lock file cannot be created or the filesystem does not implement locking, opens the vault **read-only** with `Access::ReadOnly` and no lock held; §4.5 and §4.8 both require read-only media to open, so refusing here would make the operation that diagnoses a failing drive the one operation a failing drive cannot run. Reporting the second as the first would send a user hunting for a second window that does not exist, which is FR-2's conflation in a different place.

---

## 3. Cryptographic Construction

All primitives are published and widely reviewed (HC-6). All key material lives in types that implement `ZeroizeOnDrop`.

### 3.1 Key hierarchy

The password protects a master key which protects everything else (A-6), so a password change rewrites 32 bytes rather than the vault (FR-4), and further unwrap paths can be added later without re-encrypting content (§2.2 of Requirements).

```
password ──Argon2id(salt, params)──▶ KEK
                                      │
                     wrapped_master_key│ unwrap (AEAD, AAD = header)
                                      ▼
                                     MK  (32 random bytes, generated at creation)
                                      │
                        ┌─────────────┴─────────────┐
                  HKDF-SHA256                  HKDF-SHA256
                "veil2:index:v1"            "veil2:entry-wrap:v1"
                       │                             │
                   index_key                    entry_wrap_key
```

- **KEK** — Argon2id over the password and the header salt, parameters from the header (HC-5).
- **MK** — 32 bytes from the OS CSPRNG at vault creation, never derived from the password.
- **Wrapping** — `XChaCha20-Poly1305(KEK, nonce, MK)` with **the entire preceding header as associated data**. Any tampering with the format version, the KDF parameters, or the salt therefore causes unwrap to fail. This is why the header needs no separate MAC, and it closes the parameter-downgrade path.
- **Subkeys** — HKDF-SHA256 from MK with distinct, versioned `info` strings. Domain separation is explicit: the original Veil used one key for the header, the metadata, and every file, which is the condition that makes any nonce mistake catastrophic.

Vault creation enforces C-4's minimum password length; it is the only credential policy, since strength estimation is a promise about an attacker's resources that the Design Guideline forbids the product from making.

**A wrong password and a damaged header both fail the same unwrap, and FR-2 requires them told apart.** Making the whole header associated data means the AEAD result alone cannot distinguish them — the two are indistinguishable to the cipher by construction. The rule is therefore stated here rather than left to an implementation habit:

> A header that fails its own integrity check is **damage**. A header that passes it and still fails the unwrap is a **wrong password**.

The header carries a 4-byte non-cryptographic checksum over its own preceding bytes for this purpose alone (§4.2). *Honesty clause:* the checksum is **not** a security control and defends nothing. An attacker altering a header recomputes it trivially, and the AEAD is what enforces HC-3 — unchanged. What the checksum distinguishes is *accidental* damage from a mistyped password, which is the whole content of FR-2: a user with a typo must not be sent to look for a corrupted file, and a user with a failing drive must not be told to try their password again.

**MK has exactly one unwrap path.** HC-7's unrecoverability is therefore a property of the key hierarchy rather than a policy layered over it — there is no escrow slot to disable and no second wrapping to forget to remove. Adding a further unwrap path is the deferred door of Requirements §2.2, and A-6 is what keeps it cheap.

### 3.2 Per-entry keys

Each entry gets its own random 32-byte data key, generated at ingest and stored in the encrypted index, wrapped under `entry_wrap_key`.

Deriving per-entry keys deterministically from the entry id would be simpler, but storing them buys three things cheaply, at 32 bytes each: replacing an entry (FR-13) mints a new key rather than reusing one, deleting an entry never requires rekeying anything else, and a future per-file sharing feature has a natural unit to hand over. The index already carries per-entry state, so the storage cost is marginal.

### 3.3 Content encryption

**`StreamBE32<XChaCha20Poly1305>` — the STREAM construction from the `aead-stream` crate.** This is the direct fix for the defect demonstrated in the original Veil, where the final chunk of a file could be removed and decryption still reported success. STREAM tags the last chunk distinctly, so truncation at a chunk boundary fails authentication (HC-3).

- **Chunk size: 1 MiB.** Initial; tune with use. Balances per-chunk tag overhead (16 bytes, negligible at this size) against cancellation latency and memory (S-1).
- **Nonce prefix: 19 random bytes per entry**, stored in the index entry. STREAM consumes the remaining 5 bytes of the 192-bit nonce for its counter and last-block flag. A fresh random prefix per entry, with a key that is itself unique per entry, makes nonce reuse structurally impossible rather than dependent on a counter being managed correctly.
- **Associated data per chunk includes the entry id**, so a chunk cannot be transplanted from one entry to another — the substitution case named in HC-3. STREAM already binds chunk position.
- **Content hash: BLAKE3** over the plaintext, stored in the index. Used to verify a completed extraction end to end (FR-18) and to detect a source file that changed during a long ingest.

Note that authentication is per chunk and is therefore verified as data is read, not at open. Requirements §7 states this limitation; the Spec does not paper over it.

### 3.4 What is deliberately not done

**Memory is not locked (`mlock`/`VirtualLock`).** Requirements §7 already declines to defend against memory capture on a running machine, so locking would buy no guarantee the product claims. Its behaviour and limits differ substantially across the three platforms, and a partial defence described as a defence is worse than none. Key types are zeroised on drop, which is cheap and honest.

**Plaintext is not compressed before encryption.** Compression ratios leak information about content, and the media workload Veil2 targets is already compressed. Revisiting this would require a stated reason and a fresh look at the leak.

---

## 4. Data Model and On-Disk Format

### 4.1 Vault layout

A vault is a directory. Each entry is stored as its own file under `entries/`, named by its entry id (§4.3).

```
MyVault.veil/
├── veil.header        # plaintext, fixed size
├── index.a            # encrypted index, slot A
├── index.b            # encrypted index, slot B
├── veil.lock          # advisory lock target; carries no vault data
└── entries/
    ├── 00000001.entry
    ├── 00000002.entry
    └── …
```

This is what makes A-5 true — reading one entry never touches another — and what makes S-3's damage isolation direct: damage to one entry's file cannot affect any other file. Deleting an entry removes its file; there is no shared container to rewrite or compact.

On macOS the `.veil` extension is registered as a **document bundle**, so Finder presents the directory as a single opaque document and double-click opens the application. Windows and Linux show a folder; §8.2 covers the packaging consequence. This recovers most of the single-file feel the directory layout costs.

### 4.2 Header

Plaintext, fixed size, authenticated as associated data by the master-key unwrap (§3.1). It contains only what a reader needs before it has a key (HC-5).

| Field | Type | Purpose |
|---|---|---|
| `magic` | `[u8; 8]` | `VEIL2\0\0\0`; identifies the format |
| `format_version` | `u16` | **The compatibility gate.** Read dispatches on this (FR-5, FR-6) |
| `writer_version` | `[u16; 3]` | Application version that last wrote the vault. **Provenance only; never gates access** (HC-5) |
| `kdf_algorithm` | `u16` | Argon2id today; the field exists so it need not be forever |
| `kdf_m_cost`, `kdf_t_cost`, `kdf_p_cost` | `u32` ×3 | Stored, never assumed — tuning defaults must not orphan vaults (HC-5) |
| `kdf_salt` | `[u8; 32]` | |
| `wrap_nonce` | `[u8; 24]` | |
| `checksum` | `u32` | Non-cryptographic checksum over every preceding byte. **Not a security control** — it exists solely so a damaged header can be told from a wrong password (§3.1, FR-2) |
| `wrapped_master_key` | `[u8; 48]` | 32-byte key plus 16-byte tag |

Format and writer versions are separate fields with separate lifecycles: many application releases may write one format version, and bumping the application must never invalidate a compatibility check.

### 4.3 Index

One CBOR document (RFC 8949, via `ciborium`), encrypted whole under `index_key` with `XChaCha20-Poly1305`.

CBOR is chosen over a compact non-self-describing encoding because it tolerates unknown fields, which is what makes the deferred migration path of Requirements §2.2 tractable — a reader can recognise and preserve what it does not understand. The size premium is irrelevant at index scale: at C-1's 65,536 entries the index is on the order of 20 MB, and §4.4 makes rewriting it cheap enough.

```
IndexDocument
├── index_version: u16
├── generation: u64          # monotonic; the external-modification detector (FR-24)
├── next_entry_id: u64       # monotonic; never decreases, never reset
└── entries: [Entry]

Entry
├── id: u64                  # also names its file: entries/{id}.entry (§4.1)
├── name: String              # NFC, UTF-8 (§4.6)
├── folder: String            # descriptive metadata, not structure (FR-8)
├── size: u64
├── source_mtime, added_at: u64
├── content_hash: [u8; 32]    # BLAKE3 of plaintext
├── wrapped_dek: [u8; 48]
└── nonce_prefix: [u8; 19]
```

Entry count and total size are computed by summing the loaded `entries` list rather than maintained as a separate persisted figure — summing at most C-1's 65,536 in-memory entries costs microseconds, which is not the cost the old design's incremental maintenance existed to avoid.

`next_entry_id` is **stored rather than derived, and that is a cryptographic requirement wearing bookkeeping clothes.** The entry identifier is bound into the DEK-wrapping nonce and into each chunk's associated data (§3.2, §3.3). Deriving the next identifier from the highest *live* entry — the obvious implementation — reissues a deleted entry's identifier the moment the highest entry is removed, and a wrapped key from the dead entry then decrypts under a live one's nonce. The counter must outlive the entries it counted, so it lives in the document. Deleting every entry does not reset it.

The entry carries no absolute source path. The original Veil stored one, which retained a fact about the user's machine that nothing needed.

### 4.4 Atomic index persistence

Two slots, `index.a` and `index.b`, each self-authenticating and carrying its generation number. A write serialises the whole index, encrypts it, writes it to **the slot holding the older generation**, and fsyncs. A read takes the slot with the highest generation that authenticates.

No rename is involved, because rename atomicity varies across platforms and filesystems while "the older slot is expendable" holds everywhere. A crash mid-write damages only the expendable slot, and the previous generation remains intact and openable (HC-4).

Rewriting the whole index on every mutation is accepted deliberately: at C-1 it costs tens of milliseconds, and it makes the durability argument a single sentence instead of a journal replay implementation. If C-1 ever rises by an order of magnitude this becomes an append-log with periodic checkpoints; the format version field is the door.

### 4.5 Entry storage and space management

Each entry is one file under `entries/` (§4.1), named by its id, holding exactly that entry's STREAM-encrypted chunks (§3.3). An entry has no size cap of its own beyond C-2; there is no spanning to describe, because a file grows to whatever size its content needs.

**Ingest** writes the new entry's file, fsyncs it and the containing directory (§4.7), then commits the index. Content is durable before the index names it (HC-4).

**Replace** (FR-13) writes the new content under a new id, fsynced the same way, then a single index generation step both points the path at the new id and drops the old one. There is never a window with zero intact versions.

**Delete** (FR-22) commits the index removal first, fsyncs it, then removes the old file — never the reverse. A crash between the two steps leaves a file the index no longer references; it never leaves an index pointing at a file that is already gone.

**A missing file that the index still references is damage to exactly that entry** (S-3), and to no other. The vault opens, that entry is reported unreadable, and every other entry stays retrievable. There is no per-pack attribution to build: with one file per entry, damage was never able to spread past its own file.

**Nothing happens at open.** Opening reads the header and one index slot, writes nothing, and does not list `entries/`. A write at open would advance `generation`, which is FR-24's entire detection mechanism (§4.4) — a vault opened from a stale copy would come away holding a number above a newer copy arriving moments later, and every later write would pass the check meant to refuse it.

**A file the index does not reference is left alone.** This is the residue of a replace or delete interrupted between its two steps, and equally of an ingest refused (C-1, C-2) or cancelled (FR-15) after its file was written but before the index committed — `add` has no rollback, so a write that does not finish still leaves its file behind. Either way it costs at most one entry's worth of space, it is visible to anyone who looks at the directory, and nothing reads or removes it automatically. Building a sweep for it was considered and declined: an index that is momentarily behind its own directory is indistinguishable from this case by construction, and removing a file on that guess risks exactly the loss HC-4 forbids, for reasons that have nothing to do with why the index fell behind.

On read-only storage — mounted image, write-protected drive, permissions that deny writing — the vault opens read-only and says so at open (FR-23). Refusing would make an interrupted operation on a drive that later became read-only into permanent data loss, which HC-4 forbids.

### 4.6 Name normalisation

- **Stored form: Unicode NFC, UTF-8.** Names are normalised on ingest. A name typed, pasted, or read from a filesystem API may arrive pre-composed or decomposed depending on its source; without normalisation, two visually identical names can differ in bytes and fail to match under FR-13.
- **Comparison is exact and case-sensitive** after normalisation. Matching is decided by the vault's own stored bytes, not by any filesystem's case-folding rules.
- **Identity is the full path**: the `folder` field and `name` together (FR-13). Two entries sharing a name in different folders are unrelated, and a replace targets exactly one of them. Matching on name alone would let an ingest into one folder silently overwrite a file in another.
- **Path separators are not stored in names.** The `folder` field holds `/`-separated segments; the separator is a serialisation detail, not a filesystem's.

---

### 4.7 Ingest and extraction

**Ingest is a copy** (FR-9). The source is opened read-only and is never modified, moved, or unlinked. Nothing in `veil-core` deletes a file outside a vault.

**Folder ingest walks regular files only** (FR-10). Symbolic links are not followed and are recorded as skipped (FR-11); following them risks cycles and captures data outside the tree the user selected. Each file's `folder` metadata is the added root's own name, followed by its path relative to that root, normalised per §4.6 — the root's name is the top-level segment, not discarded, so two different added folders that happen to share a name at some inner level still store distinct identities.

**Both directions stream** (A-2, S-1, FR-21). The source is read in chunk-sized reads, each chunk encrypted and appended to the entry's own file; extraction reads that file sequentially and writes decrypted chunks to the caller's `Write`. Neither direction holds more than a small constant number of chunks, so peak memory is independent of file size at C-2's 64 GiB maximum. BLAKE3 is computed in the same pass, so hashing costs no extra read.

**Ordering is what makes FR-12 true.** The entry's file is written and fsynced *before* the index generation advances and is fsynced. A crash between the two leaves an entry file that no index references — left alone, per §4.5 — and never an index entry pointing at content that was not durable. Success is reported only after the index fsync returns.

**A file's name is durable only when its directory is.** `fsync` on a file makes that file's *contents* durable; the directory entry that gives those contents a name lives in the parent directory and needs its own sync. Without it a crash can leave a fully-synced entry file that nothing can find, or a header renamed on one machine and not on another — a durable file with an undurable name, which fails FR-12 while every fsync in the code returned successfully. **The containing directory is therefore inside the same ordering obligation as the file**, and is synced after any operation that creates, renames over, or removes a file within it: a newly created entry file, the header's initial write, an index slot's first creation, and the file removal that follows a delete or replace.

**Replace** (FR-13) writes the new entry to completion and durability first, then advances one index generation that simultaneously points the name at the new entry and drops the old one; the old file is removed afterward (§4.5). There is no window in which zero intact versions exist.

**Cancellation** (FR-15) is checked at chunk boundaries. Because the index has not advanced, cancelling an ingest leaves only an unreferenced entry file, and the vault is indistinguishable from one where the operation never started — which is precisely what the Design Guideline promises the user when it says so.

**Extraction verifies before it succeeds** (FR-17, FR-18). Chunk authentication fails fast on tampering, and the BLAKE3 hash is compared against the index after the final chunk. On either failure the partial output is removed and the error names the affected entry.

### 4.8 Verification

Verification (FR-26) reuses the extraction path of §4.7 with the output discarded: every entry's chunks are decrypted and authenticated in order, and the BLAKE3 hash is compared against the index. Nothing is written, so verification runs on a read-only vault and takes no more than a shared lock.

Failure is per entry, not per vault. A failing entry is recorded and verification continues, so a vault with several damaged entries yields a complete list of what failed rather than stopping at the first casualty — which is the attribution S-3 requires and what §8.6 of the Design Guideline presents.

Progress is reported per entry rather than per byte, because the Design Guideline's estimate is in time and entry counts are what a user can hold in their head. Cancellation returns the entries verified so far and their results; a partial verification is a partial answer, not a discarded one.

Verification reads the entire vault and is therefore never scheduled, never automatic, and never triggered at open (FR-26).

---

## 5. Application Layer

### 5.1 Core API

The shape `veil-core` exposes, and the constraints it satisfies. Signatures are illustrative.

```rust
Vault::create(path, password, params) -> Result<Vault>      // FR-1
Vault::open(path, password) -> Result<Vault>                // FR-2, FR-5, FR-6
Vault::change_password(&mut self, old, new) -> Result<()>   // FR-4, rewraps MK only
Vault::lock(self)                                            // FR-3, zeroises

Vault::entries(&self) -> &[Entry]                            // FR-7, from memory
Vault::find(&self, folder, name) -> Option<&Entry>           // FR-13, full-path identity (§4.6)
Vault::statistics(&self) -> Statistics                       // derived from entries(), see below
Vault::access(&self) -> Access                               // FR-23, read-only stated at open
Vault::limits(&self) / set_limits(&mut self, Limits)         // C-1, C-2, FR-16

Vault::unreadable_entries(&self) -> Vec<EntryId>             // S-3, the attribution, no content read

Vault::add(&mut self, src, folder, &mut dyn Progress, &Cancel) -> Result<EntryId>
Vault::replace(&mut self, id, src, …) -> Result<EntryId>     // FR-13
Vault::extract(&self, id, dst: &mut dyn Write, …) -> Result<()>
Vault::extract_to_path(&self, id, path, …) -> Result<()>     // FR-18, removes partial output
Vault::delete(&mut self, id) -> Result<()>                   // FR-22
Vault::reload(&mut self) -> Result<()>                       // FR-24, adopts an external change
Vault::verify(&self, &mut dyn Progress, &Cancel) -> Result<VerifyReport>  // FR-26, §4.8
```

`extract` writes to a `Write` rather than returning bytes, which is what makes S-1 structural rather than a discipline the caller must maintain. It therefore never learns a path — and FR-18's "the incomplete output is removed" can only be honoured by whoever created the file. `extract_to_path` is that owner, and it lives here rather than in each frontend: writing the removal twice is how two frontends come to differ, which A-4 forbids.

**`statistics` is derived, not maintained.** Entry count and total size are computed by summing the resident `entries` list on call — cheap at C-1's scale, and not a distinct requirement in its own right. It exists as a method only so both frontends compute the same figures the same way (A-4), not because anything needs to cache them.

**`limits` and `set_limits` make C-1 and C-2 values the API accepts, not compile-time constants** (FR-16). A test exercising the limit-refusal path can set a small limit rather than writing 64 GiB to reach it.

**`unreadable_entries` is S-3's attribution, computed on call, never at open.** One existence check per entry's file, no content read, so a vault with a missing file opens at the same speed as one without, and the casualties are enumerable the moment anything asks.

`reload` is the second half of FR-24. Detecting an external change and refusing to write over it is only useful if there is a way forward; requiring the password again to get one would make the safe answer cost more than the unsafe one, which is how a safety mechanism becomes something users route around. The subkeys are already held, so a reload is one index read.

The whole index is resident once opened, so browsing is memory-speed (FR-7) and statistics cost nothing beyond summing what is already resident.

**FR-28, FR-29, and FR-30 add no core surface.** Per-entry detail (FR-28) is fields `entries()` already returns — `source_mtime` alongside the rest, not previously surfaced past this layer. Extension grouping (FR-29) is a derivation over `name`, computed where each frontend already holds the full entry list, the same way a frontend computes its own display formatting today (`formatSize`, `formatAdded` in the GUI; the table renderer in the CLI) with no core equivalent. Preview (FR-30) is `extract` (§4.7) with `dst` a memory buffer instead of a file — the signature above already takes `&mut impl Write`, and a `Cursor<Vec<u8>>` satisfies that trait without a new method. No format version bump accompanies any of the three: nothing changes about what is stored, only about what an already-resident field or an already-existing read path is used for.

### 5.2 Command-line application

`clap` for parsing. Output follows Design Guideline §3.4: human-readable table by default with the GUI's column order, machine-readable on request, progress to standard error and results to standard output so pipelines stay clean, and progress degrading to periodic lines when not attached to a terminal.

Non-interactive invocation is detected rather than assumed: where a password is required and no terminal is available, the command fails naming the missing input instead of blocking on a prompt. The password may be supplied by an environment variable or a file path for scripted use, never as a command-line argument — arguments are visible in process listings and shell history.

Exit codes distinguish the §6 error classes, so scripts can tell a wrong password from a damaged vault without parsing text. Verification (§4.8) exits non-zero when any entry fails, so it is usable as a check in a backup script without parsing its output.

The codes are fixed here rather than in the application, because the moment a backup script tests for one it is an interface with a compatibility obligation, and this document is where those live. The mapping is exhaustive over the §6 taxonomy: a variant added later without a code assigned is a defect, not a fallback into 1.

| Code | Condition | Satisfies |
|---|---|---|
| 0 | Success | |
| 1 | Unexpected failure | |
| 2 | Usage error, including `PasswordTooShort` | C-4 |
| 3 | `WrongPassword` | FR-2 |
| 4 | `NotAVault`, `FormatTooNew`, `FormatSuperseded` | FR-5, FR-6 |
| 5 | `Corrupt` — damage found, including by verification | HC-3, FR-26, S-3 |
| 6 | `VaultInUse` | FR-23 |
| 7 | `ChangedOnDisk` | FR-24 |
| 8 | `ReadOnly` | §4.5, §4.8 |
| 9 | `LimitExceeded` | FR-16 |
| 10 | `Cancelled` | FR-15, FR-20 |
| 11 | `StorageUnavailable` | FR-25 |
| 12 | A required password was not supplied and no terminal is available to ask | §5.2 above |
| 13 | `NotFound`, `AlreadyExists` | FR-14 |

Verification may be run from a scheduled script: it only reads the vault and modifies nothing, so automating it carries none of the risk automatic writes would.

**Two additions to the command surface for v2.1, held to the same compatibility rule as the exit codes above:**

- **`veil detail <vault> <file>`** (FR-28) — prints one entry's complete recorded metadata: name, folder, size, the source's own modification time (labelled `Modified`, matching Design Guideline §8.9), and when it was added. `file` is folder and name together, the same identity argument `save-copy`, `replace`, and `delete` already take. `NotFound` (13) if nothing matches.
- **`--group` on `list` takes an optional value** (FR-29). Today it is a bare boolean flag; changing it to require a value would break every script that passes it bare. Instead: omitted, the listing is flat, exactly as before this change; given bare (`--group`), it groups by folder — the flag's existing behavior, unchanged; given `--group=extension`, it groups by the rule FR-29 defines (the substring of `name` after its last `.`, a leading dot not counting as one). **Table output for a bare `--group` is unchanged byte-for-byte.** JSON output is the one exception to "no script observes any difference": before this change `--format json` ignored `--group` entirely and always printed a flat `{"files": [...]}` — undocumented and untested, so this is a gap closed rather than a behavior preserved. `--group --format json` now returns `{"groups": [...]}`, the same shape `--group=extension --format json` uses.

Preview (FR-30) has no CLI form, for the reason Design Guideline §3.4 gives: there is no terminal surface to preview onto, and the capability it presents a view of — extraction — already has one in `save-copy`.

### 5.3 Graphical application

**Tauri v2.** Rust backend linking `veil-core` directly, web frontend in the system webview.

The alternative considered was egui. It was rejected on evidence: egui's text layout lacks complex-script shaping, and the known workarounds — pre-rendering to a texture, or wrapping a webview — either reimplement the problem or reintroduce the dependency. Veil2's entire interface is user-supplied filenames, which may be in any script the user's files use, and a toolkit that cannot render them correctly is disqualified regardless of its other merits. Correct rendering of the user's own filenames is not a refinement here; it is the product working.

Vault operations run on a worker thread and report progress to the UI thread through Tauri's event channel, satisfying A-3 without blocking the webview.

**Webview persistence.** A system webview may cache data to disk by default, including rendered filenames — which HC-1 requires stay undisclosed without the password. The webview is configured for ephemeral storage (on macOS, `WKWebView` with a non-persistent `WKWebsiteDataStore`), and the frontend uses no `localStorage`, `sessionStorage`, or IndexedDB; Content-Security-Policy is restricted to the bundled origin, and developer tools are compiled out of release builds. This is ordinary configuration, not a gated feature: the worst a lapse here leaks is filenames, not content, and it carries no dedicated release gate or mandatory per-platform verification.

**Accepted cost.** Tauri brings a JavaScript toolchain and its dependency tree into a security product's supply chain, which egui would not have. It is bounded by the same policy as §7: pinned versions, audited by the same gates, and no frontend dependency permitted to make network requests at runtime.

**Detail, grouping, sorting, and multi-select (FR-28, FR-29) are frontend state over data the list already holds.** `EntryInfo` gains one field, `source_mtime`, alongside the ones it already serializes — everything FR-28's detail panel needs. Extension grouping, column sort, and multi-select touch no Tauri command at all: `list_entries` already returns the complete list once per open vault (§5.1), and rearranging, grouping, or selecting within it is the frontend's own array manipulation, the same standing as the search filter that already works this way. No dependency is added on either side for any of this — no markdown parser, because Requirements FR-30 shows Markdown as plain text; no client-side state library, because the existing module-level state in `main.ts` already holds everything a few more `let` bindings (sort column, sort direction, grouping mode, a `Set` of selected ids in place of the single `selectedId`) can extend.

**`preview_entry(id) -> PreviewPayload`, the one new command, for FR-30.** Checks the entry's recorded `size` against C-5 before touching any ciphertext — a refusal for an oversized entry costs nothing and reads nothing. For an entry within the cap, calls the existing `Vault::extract` (§5.1) with `dst` a `Cursor<Vec<u8>>` rather than a file, so FR-18's verification runs exactly as it does for a save-copy; a failed check returns the same error variant a failed extraction would, and nothing is buffered past that failure.

```rust
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum PreviewPayload {
    Image { mime: &'static str, base64: String },   // jpg, jpeg, png, gif, webp, bmp
    Text { content: String },                        // txt, md, log, csv, json — shown unrendered
}
```

An extension outside FR-30's supported list, or content that is not valid UTF-8 for a text-listed extension, is refused before decryption is attempted — the same "absent, not disabled" rule the context menu item follows (Design Guideline §3.5) applies here one layer down, as a typed refusal rather than a generic failure. A `Text` variant carries the decoded string directly rather than base64, since the frontend displays it as-is; `Image` carries base64 because Tauri's IPC channel serializes command results as JSON, which has no binary type of its own — at C-5's 50 MiB cap the base64 overhead (roughly a third larger) is not a measured concern.

**Clearing preview content (FR-30, extending FR-3) is honoured to the extent each layer can honour it.** On the Rust side, the buffer passed to `extract` and the payload built from it are ordinary owned values with no `ZeroizeOnDrop` — that annotation exists for *key* material (§3.1), and decrypted file content was never a secret Veil2 tries to keep from its own process. They are simply dropped once the command's response is sent; nothing here retains a second copy. On the frontend, closing the preview, locking the vault, and quitting the application each release every reference to the previewed content — including revoking any object URL created to display an image — so nothing outlives its own visible use. What this does not claim: JavaScript offers no way to force the engine to overwrite freed memory immediately, so "cleared" here means *dereferenced promptly, in every path that ends a preview*, not zeroised on a timeline the application controls. Requirements §7 already declines to defend against memory inspection of a running, unlocked vault; this is the same limit, restated for the one new place decrypted content now transiently lives.

---

## 6. Error Handling

Typed errors via `thiserror` throughout `veil-core`. **`anyhow` is not used in the library** — the original Veil converted every failure into a single string-carrying variant, which is why a wrong password and a corrupted vault were indistinguishable to callers. Binaries may use `anyhow` at their top level.

The taxonomy distinguishes, at minimum:

| Variant | Satisfies |
|---|---|
| `WrongPassword` | FR-2 — distinct from corruption, so the user is sent to the right remedy |
| `FormatTooNew { required, supported }` | FR-5 |
| `FormatSuperseded { version }` | FR-6 |
| `Corrupt { what, affected_entries }` | HC-3, S-3 — carries which entries are affected |
| `VaultInUse`, `ChangedOnDisk`, `StorageUnavailable` | FR-23, FR-24, FR-25 |
| `LimitExceeded { limit, value }` | FR-16 — carries both numbers the message must name |
| `Cancelled { rolled_back: bool }` | FR-15, FR-20 — states what the cancel left behind |
| `VerificationFailed { entries }` | FR-26, S-3 — carries every failing entry, not just the first |
| `ReadOnly` | §4.5, §4.8 — the vault opened without a lock because its storage would not take one, and a write was attempted |
| `NotFound` | FR-2's principle applied to naming: a path that matches nothing is a mistyped name, not damage |
| `AlreadyExists` | FR-14 — the path is already a file's identity, and adding a second is refused |
| `PasswordTooShort { minimum }` | FR-1, C-4 — raised where a password is set, never where one is offered to open a vault |

Two prohibitions, each with its reason:

- **No error, `Display`, or `Debug` output contains plaintext, file content, key material, or the password** (HC-2). Key types have hand-written `Debug` implementations that print a placeholder.
- **Logging never records entry names, folder metadata, or content** (HC-1). `tracing` is used for operational events only — operation started, bytes processed, error variant. A log file that reconstructs the index would defeat the vault.

`NotFound` is its own variant for the same reason `WrongPassword` is: a mistyped name and a damaged vault send a user to entirely different remedies, and reporting the first as the second is the original Veil's defining failure repeated at the level of names. It was reported as `Corrupt` with an empty affected list until Phase 3 found it.

Neither `NotFound` nor `AlreadyExists` carries the path, for the reason the I/O variant carries none: the caller supplied it and is the layer that can name it. It is also the only layer permitted to — a file name is index data, and HC-1 is why no variant here holds one.

`ReadOnly` is its own variant rather than an I/O error carrying a read-only kind. Both §4.5 and §4.8 require a read-only vault to *open* — refusing would turn an interrupted operation on a drive that later became write-protected into permanent data loss, and would make the operation that diagnoses a failing drive the one operation a failing drive cannot run. So the refusal happens at the write, and it is a condition the frontends must phrase differently from a disk failure: nothing is wrong, the vault simply cannot be changed from here.

Errors carry the state fact the Design Guideline's three-part message needs: `Cancelled` says whether it rolled back, `Corrupt` names the affected entries.

---

## 7. Dependencies

Locked initial set. Acceptance policy: primitives come from RustCrypto where one exists, because HC-6 requires published and widely reviewed constructions and that ecosystem is the reviewed one. No vendor SDKs. Every dependency is pinned; `cargo audit` and `cargo deny` are gates run before every commit, and a failure of either blocks the commit (§8.1).

| Crate | Purpose | Requirement |
|---|---|---|
| `chacha20poly1305` | XChaCha20-Poly1305 | HC-3, HC-6 |
| `aead-stream` | STREAM construction over the above | HC-3, HC-6 |
| `argon2` | Argon2id key derivation. Pinned to the 0.6 pre-release: 0.5 sits on the previous RustCrypto generation, and running two generations of `digest` in one graph means the audited implementation may not be the one that runs | HC-6, C-3 |
| `hkdf`, `sha2` | Subkey derivation | §3.1 |
| `blake3` | Content hashing | FR-18 |
| `zeroize` | Key material lifetime | §3.1 |
| `ciborium`, `serde` | Index serialisation | §4.3, FR-6 |
| `rand`, `getrandom` | CSPRNG for keys, salts, nonces | §3.1 |
| `unicode-normalization` | NFC normalisation of stored names (§4.6) | FR-13 |
| `fs4` | Cross-platform advisory locks | FR-23 |
| `thiserror` | Error taxonomy | §6 |
| `tracing` | Operational logging, subject to §6 | |
| `clap` | CLI argument parsing | A-4 |
| `anyhow` | Error handling at a binary's top level only. §6 forbids it in the library and that prohibition stands — the reason it exists is that the original Veil made a wrong password and a damaged vault indistinguishable, which is a property of the library's taxonomy, not of how a binary prints one | §6 |
| `serde_json` | Machine-readable command-line output | Design §3.4 |
| `ctrlc` | Interrupt handling, so an interrupt reaches the cancellation the core already implements rather than killing the process | FR-15, FR-20 |
| `rpassword` | Reading a password from a terminal without echoing it | HC-2 |
| `tauri` (v2) | GUI shell, webview integration, native dialogs | §5.3 |

The frontend toolchain is pinned and lockfile-committed like the Rust dependencies, and audited by the same gates. No frontend dependency may make a network request at runtime; the Content-Security-Policy of §5.3 enforces this rather than relying on review.

**The table above covers dependencies that ship.** Test-only dependencies are held to the same pinning and audit policy but are listed separately, because a crate that cannot reach a release binary is a different risk: `proptest` (property tests, §9), `assert_cmd` (CLI tests, §9), `tracing-subscriber` (the logging guard of §6), and `serde_json` in `veil-core`'s tests only, where it is the readable form for fixtures and never a serialisation path the library uses — §4.3 fixes CBOR for that. Adding one still requires a bump of this section, so the set stays deliberate.

The original Veil's `sled` is not carried forward: it has been unmaintained since 2021, and §4.3 and §4.4 do the job in a few hundred lines with a durability story that fits in a paragraph.

**Nothing is added to this table for v2.1.** §5.1 and §5.3 above account for why: FR-28 and FR-29 are derivations over data and dependencies already present, and FR-30 reuses `extract` and the standard library's own `Cursor`, deliberately kept out of Markdown rendering (a dependency and a risk both) by Requirements FR-30's plain-text decision. A feature round that stayed inside the existing dependency set was not the goal going in; it fell out of reusing `extract` for preview rather than inventing a second read path.

---

## 8. Build and Release

### 8.1 Toolchain

Rust 2024 edition. Release versions begin at 2.0.0 per Requirements §8.

No CI pipeline. The gates run locally before every commit: `cargo fmt --check`, `cargo clippy --workspace --all-targets` at `-D warnings`, `cargo test`, `cargo deny check`, `cargo audit`, and, once `crates/veil-gui/ui` exists to have one, `npm audit` — §7's "audited by the same gates" for the frontend toolchain meant this, and this section is where it should have been named alongside the Rust gates rather than left implicit. All must pass.

**macOS is the only platform this ships for, builds for, or is run on** (Requirements §2.1). Windows and Linux are not built and not scheduled (§2.2); if either is taken up later, it is evaluated and verified on its own machine at that time, with no portability guarantee assumed in advance.

### 8.2 Packaging

**Ships with 2.0:**

- **macOS** — `.veil` registered as a document bundle via a declared UTI, so the vault reads as a single document in Finder (§4.1). Application signed and notarised.

### 8.3 Reproducibility

Builds are reproducible to the extent the toolchain allows, and release artifacts carry checksums. Users are asked to trust a binary that guards their data; being able to rebuild it is part of earning that.

---

## 9. Testing Strategy

The original Veil had fourteen unit tests, no integration tests, and logic that could not be exercised without a pseudo-terminal. A-1 removes that obstacle, and this section is where the requirements become executable.

**Unit tests** per module, colocated.

**Integration tests against `veil-core` directly** — no process, no terminal, no prompts.

**CLI tests** via `assert_cmd`, covering the full command surface as A-4's parity claim requires.

**Property tests** (`proptest`): any byte sequence, at any length including zero, survives ingest and extraction byte-identically.

**Adversarial corruption suite — one test per HC-3 failure mode, and this suite is not optional:**

| Mutation | Must |
|---|---|
| Flip one byte of ciphertext | Fail authentication |
| Truncate the final chunk | Fail — *direct regression test for the demonstrated defect* |
| Truncate mid-chunk | Fail |
| Reorder two chunks | Fail |
| Transplant a chunk between entries | Fail |
| Tamper with header KDF parameters | Fail master-key unwrap |
| Corrupt one entry's file | Fail **only** that entry, and name it (S-3) |

**Crash tests for HC-4:** kill the process mid-operation during `add`, `replace`, and `delete`, then assert the vault opens, the index authenticates, statistics match a full recount, and no entry that existed beforehand is lost. A real kill, not a simulated one — an abstraction inside `veil-core` that let a test pretend to crash would add a seam to shipped code to serve a test, and would be testing the pretence. The kill is a kill and not an interrupt: an interrupt is cancellation, which is a different guarantee with its own tests (FR-15). All three are killed through the shipped `veil` binary; nothing left needs a multi-gigabyte setup to reach, so no test-only subject exists.

*What the crash tests cannot reach, stated rather than implied:* killing a process does not empty the operating system's page cache. A vault surviving every kill here has proved the **ordering** holds — that no index generation names bytes the code had not yet synced — and has not proved that the platform's `fsync` reaches the platter. Whole-machine power loss is the only test for that and there is no rig for it (§11.1).

**The header and index parsers get randomised input** — they are the only attacker-controlled inputs reachable before authentication, so a panic in either is a defect regardless of who reaches it. `cargo-fuzz` would be the right tool and is declined (§11.1): it needs a nightly toolchain and a tool on the machine. What runs instead is seeded and deterministic, so a failure reproduces exactly, and it covers the same two entry points at lower depth.

**Scale tests**, marked `#[ignore]` and run on request, since a multi-gigabyte fixture costs minutes and disk: a multi-gigabyte entry, and a vault at C-1's entry limit, asserting S-1 (peak memory does not scale with file size) and S-2 (open time does not scale with vault size).

**Added for v2.1:**

- **Extension derivation**, against one fixture list of name/expected-extension pairs covering `archive.tar.gz` → `gz`, `.gitignore` → none, `README` → none, and ordinary cases — run against both the CLI's Rust implementation and the GUI's TypeScript one, so the two are checked against the same cases even though neither calls the other's code (§5.1's note on why this is duplicated rather than shared).
- **`preview_entry` never touches disk.** An integration test snapshots the vault directory's contents before and after a preview call and asserts they are identical — the direct check for FR-30's "memory only, never a temporary file," in the same spirit as the adversarial suite's direct regression tests above.
- **`preview_entry` refuses above C-5** without reading the entry's stored ciphertext at all — asserted by pairing the refusal with a corrupted entry that would fail FR-18's check if it were ever read, and confirming the corruption is never reported.
- **`detail` and `--group=extension`** get the same CLI test treatment (`assert_cmd`) as every other subcommand (§9 above), including that a bare `--group` still behaves exactly as it did before this section existed.

---

## 10. Milestones

High-level; the Implementation Plan expands each into phases and tasks. Each states what it proves.

**M1 — Format and crypto core.** Header, key hierarchy, STREAM content encryption, entry file write and read, index persistence, name normalisation (§4.6). *Proves the format and the cryptographic construction, and that tampering and truncation fail loudly — the adversarial suite of §9 passes before anything is built on top.*

**M2 — Vault operations.** Add, list, extract, replace, delete over the §5 API with progress and cancellation. *Proves the core API is sufficient for both frontends without either existing yet.*

**M3 — CLI at full parity.** *Proves the core is usable with no UI, and establishes the integration test surface A-4 depends on.*

**M4 — Durability.** Crash-injection suite for `add`, `replace`, and `delete`. *Proves HC-4 — that no single interruption leaves a vault unopenable or loses data that existed beforehand.*

**M5 — GUI foundation.** Tauri shell over `veil-core`, ephemeral webview storage configured per §5.3, and the entry list rendering complex-script filenames correctly in both themes with OS drag-and-drop and native dialogs working. *Proves that the interface renders the user's own filenames correctly — the one thing it exists to do — before any feature is built on it.*

**M6 — GUI v1, and 2.0.0 for macOS.** The Design Guideline realised, packaged per §8.2. *Proves the product.*

**M7 — Browsing screen additions, and 2.1.0.** The context menu, per-entry detail, extension grouping, column sort, multi-select, and preview (FR-28–FR-30) over the M6 GUI and its CLI peer. *Proves that the browsing screen can grow real capability — a second read path (preview) included — without a format version bump, a new core dependency, or a break in an existing script's use of `--group`.*

M1 through M4 touch no GUI code, so the interface work blocks nothing and is blocked by nothing until M5. M7 touches no format or crypto code at all — every check in §9's adversarial and crash suites from M1 and M4 still covers the whole of what a vault is on disk, unchanged.

---

## 11. Open Items

- **Final Argon2id parameters.** `m = 256 MiB, t = 3, p = 4` is what new vaults are created with. **Measured on the development machine at 0.27 s in a release build** (Apple silicon, macOS; `cargo test -p veil-core --test kdf_cost -- --ignored --nocapture` reproduces it, and prints the neighbouring parameter sets for comparison). That is comfortably inside C-3's one-second budget on fast hardware, which is the wrong end of the range to measure: C-3's target is the *weakest* supported machine, and this one is among the strongest. The item stays open for that reason and that reason only — one number is now known rather than none. Nothing is orphaned by a later change: HC-5 means every vault records what it was created with, and opening reads that and never a constant. Resolver: the same measurement on low-spec hardware when available.
- **Whether the platform's `fsync` reaches the platter.** The write ordering is proved by crash-injection (M4): no index generation names bytes the code had not yet synced. Whether the underlying platform honours `fsync` all the way to the medium is unverified — that needs whole-machine power loss, and there is no rig for it. Stated as an acknowledged gap rather than closed by pretending otherwise.
- **Fuzzing the header and index parsers.** `cargo-fuzz` would be the right tool; declined because it needs a nightly toolchain and a tool on the development machine. Seeded, deterministic randomised testing covers the same two entry points at lower depth and runs with the ordinary suite (§9).
- **Format-version support is not withdrawn while Requirements §2.2's migration path remains unbuilt.** A release may not refuse a vault it can still technically read; there would be no other route by which that vault's data could be recovered.
- **Whether extension derivation should move into `veil-core` if a third consumer ever needs it.** Declined for now (§5.1): two consumers each implementing one written rule, checked against a shared fixture list (§9), was judged cheaper than a shared crate for a one-line string operation. The same tension is already open in Design Guideline §9 for shared vocabulary strings; this is the same question in a different place, not a new one. Resolver: revisit if a third frontend appears.
- **Whether a base64 `Image` payload stays comfortable at C-5's 50 MiB cap**, or whether `preview_entry` should move to a dedicated byte-stream response if IPC overhead proves noticeable in practice. Resolver: measured once M7 is built, the same way the KDF cost parameters above were measured rather than assumed.
