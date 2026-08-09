# Veil2 — Technical Specification

**Version:** 2.0
**Status:** approved
**Date:** 2026-08-09
**Owner:** wattanit
**Companion documents:**
- Requirements Document v2.0 — upstream
- Design Guideline v2.0 — upstream

*Changes since v1.4 (**major — a requirement withdrawn and a decision reversed**):* absorbs all ten of Phase 4's notes to upstream, plus two of Phase 2's that were never absorbed.

> **The versions in earlier commits are wrong, including v1.5 published earlier the same day.** Every version up to v1.4 describes reconciliation at open as a write. v1.5 described it as a read that reports. Both are wrong: nothing happens at open at all. Read none of them as current; §11.2 keeps the trace of how the error was made and found.

What changed:

- **§4.5 loses reconciliation at open entirely.** Following FR-32's *withdrawal* in Requirements v2.0. Opening a vault reads the header and one index slot and does nothing else: it writes nothing, and it does not walk the packs directory. The paragraph that made reconciliation "a write during open, conditional on the vault being writable" is gone, and so is the mechanism it described. Finding space an interrupted operation left is now done by the two operations that already walk the packs and that a user asks for — reclaiming, and reporting the figures.
- **§8.1, §8.2, §9 and §10 stop claiming three platforms.** macOS is what 2.0 ships and what the suite is run on; Windows and Linux are built for and unverified, and each becomes a release when it has been run. §2.1 of Requirements carries the scope decision.

What is additive: **§4.3 gains `next_pack_id`** — pack identifiers must not be reused after reclaiming removes the highest pack, for the reason entry identifiers must not be; §4.7 puts the containing directory inside the durability ordering, which it never did and which three phases did not notice; §2 states what happens when storage will not take a lock at all; §5.1 gains the two accessors S-4 depends on and states that `recount_statistics` is where the on-disk truth comes from; §9 says what the crash tests kill; §11.1 closes two open items and re-words a third. §6 gains `PasswordTooShort`. §3.1 already required vault creation to enforce C-4's minimum; nothing did, because the taxonomy had no way to say so. It is enforced in the library rather than in each application, since two frontends applying their own minimum is how one of them ends up with a weaker one (A-4).

*Changes since v1.2 (minor — additive, no decision reversed):* absorbs what Phase 3 needs before it can start. §5.2 gains the exit-code table, which a script depending on it makes a compatibility obligation; §6 gains `NotFound` — naming nothing was being reported as damage, which is FR-2's conflation one level down — and `AlreadyExists` for FR-34; §7 gains the four dependencies the command line ships.

*Changes since v1.1 (minor — additive and clarifying, no decision reversed):* absorbs what Phases 1 and 2 discovered. §3.1 and §4.2 state how a wrong password is told from a damaged header, and the header gains the checksum that makes it possible; §4.3 names the entry-identifier counter; §4.5 and §11.1 make the pack cap and the C-1/C-2 limits values the API accepts; §5.1 gains `extract_to_path` and `reload`; §6 gains the read-only condition; §7 states that its table covers runtime dependencies and names the test-only ones; §8.1 and §9 remove the continuous-integration matrix and state plainly that cross-platform behaviour is therefore unverified; §11.1 resolves two items and records two decisions.

*Changes since v1.0 (minor — additive and clarifying, no decision reversed):* §4.6 fixes replace-matching to the full path; §4.8 added for verification; §5.1 and §5.2 gain the operation; §6 gains its error variant.

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
| `store` | Pack files, extent allocation, space accounting, compaction |
| `index` | Entry model, atomic index persistence, statistics |
| `vault` | Public API, orchestration, locking, progress and cancellation |
| `error` | Typed error taxonomy |

`crypto` is **not** split into a separate crate initially. Splitting it later is a mechanical move; splitting it now creates cross-crate churn while the format is still moving. It is kept free of dependencies on the other modules so the split stays cheap if independent audit becomes worthwhile.

---

## 2. Execution Model

**`veil-core` is synchronous and blocking. No async runtime.** Every operation is file-I/O bound, and a blocking API is callable from any caller — the CLI drives it directly, the GUI drives it on a worker thread. Requiring an async runtime would impose a large dependency and a colouring constraint on consumers for no benefit at this workload.

**Long operations take a progress sink and a cancellation token** (A-3). Cancellation is cooperative and checked at chunk boundaries, which bounds cancellation latency to one chunk of I/O. Both are parameters rather than global state, so the CLI can pass no-ops and the GUI can marshal to its UI thread.

**An open vault is a value, not a singleton** (A-7). `Vault` is `Send` and not `Sync`: one writer at a time within a process, which matches the single-writer guarantee the on-disk format assumes. Supporting several open vaults later is a caller-side change only.

**Advisory locking** (FR-26) uses an OS advisory lock held on `veil.lock` inside the vault directory (§4.1) for the lifetime of the open vault. *Honesty clause:* advisory locks are unreliable on network filesystems (NFS, SMB) and some FUSE-backed mounts. On those the lock is best-effort and the generation counter of §4.3 is the actual protection — Veil2 detects the conflicting write and refuses rather than preventing it (FR-27). The product says so when a vault is opened from a network path.

**Failing to take the lock and being refused the lock are different conditions** (FR-26). Contention — another process holds it — is `VaultInUse`. Being unable to take one at all, because the lock file cannot be created or the filesystem does not implement locking, opens the vault **read-only** with `Access::ReadOnly` and no lock held; §4.5 and §4.8 both require read-only media to open, so refusing here would make the operation that diagnoses a failing drive the one operation a failing drive cannot run. Reporting the second as the first would send a user hunting for a second window that does not exist, which is FR-2's conflation in a different place.

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
- **Content hash: BLAKE3** over the plaintext, stored in the index. Used to verify a completed extraction end to end (FR-17) and to detect a source file that changed during a long ingest.

Note that authentication is per chunk and is therefore verified as data is read, not at open. Requirements §7 states this limitation; the Spec does not paper over it.

### 3.4 What is deliberately not done

**Memory is not locked (`mlock`/`VirtualLock`).** Requirements §7 already declines to defend against memory capture on a running machine, so locking would buy no guarantee the product claims. Its behaviour and limits differ substantially across the three platforms, and a partial defence described as a defence is worse than none. Key types are zeroised on drop, which is cheap and honest.

**Plaintext is not compressed before encryption.** Compression ratios leak information about content, and the media workload Veil2 targets is already compressed. Revisiting this would require a stated reason and a fresh look at the leak.

---

## 4. Data Model and On-Disk Format

### 4.1 Vault layout

A vault is a directory. This is what makes S-3, S-4 and FR-25 achievable at the sizes in Requirements §1 — a single monolithic container fails all three: it re-uploads in full on any change, loses everything to one bad region, and cannot be compacted without whole-vault free space.

```
MyVault.veil/
├── veil.header        # plaintext, fixed size
├── index.a            # encrypted index, slot A
├── index.b            # encrypted index, slot B
├── veil.lock          # advisory lock target; carries no vault data
└── packs/
    ├── 000001.pack
    ├── 000002.pack
    └── …
```

On macOS the `.veil` extension is registered as a **document bundle**, so Finder presents the directory as a single opaque document and double-click opens the application. Windows and Linux show a folder; §8.2 covers the packaging consequence. This recovers most of the single-file feel the directory layout costs.

### 4.2 Header

Plaintext, fixed size, authenticated as associated data by the master-key unwrap (§3.1). It contains only what a reader needs before it has a key (HC-5).

| Field | Type | Purpose |
|---|---|---|
| `magic` | `[u8; 8]` | `VEIL2\0\0\0`; identifies the format |
| `format_version` | `u16` | **The compatibility gate.** Read dispatches on this (FR-5, FR-30) |
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
├── generation: u64          # monotonic; the external-modification detector (FR-27)
├── statistics                # maintained incrementally, never scanned (FR-22)
│   ├── entry_count, logical_bytes, physical_bytes, reclaimable_bytes
├── next_entry_id: u64        # monotonic; never decreases, never reset
├── next_pack_id: u32         # monotonic; never decreases, never reset (§4.5)
└── entries: [Entry]

Entry
├── id: u64
├── name: String              # NFC, UTF-8 (§4.6)
├── folder: String            # descriptive metadata, not structure (FR-7)
├── size: u64
├── source_mtime, added_at: u64
├── content_hash: [u8; 32]    # BLAKE3 of plaintext
├── wrapped_dek: [u8; 48]
├── nonce_prefix: [u8; 19]
└── extents: [(pack_id: u32, offset: u64, length: u64)]
```

`next_pack_id` is stored for the same reason and was **not** in the model until Phase 4 asked what happens when reclaiming removes the last pack. Allocating "one above the highest pack that exists" looks safe, and is, until reclaiming space removes a pack that is entirely dead — if that pack held the highest identifier, the next allocation takes the number back. A stale index slot, or an older index a sync daemon delivers late, then names a pack whose bytes are now a different pack's. That fails authentication and is reported as damage rather than returning wrong content (HC-3), so the consequence is bounded — and reporting damage where there is none is FR-2's failure exactly: the user is sent to look for a corrupted vault they do not have. A stored counter costs four bytes and removes the case. The asymmetry with pack *files*, which are named from this counter and never renumbered, is deliberate.

`next_entry_id` is **stored rather than derived, and that is a cryptographic requirement wearing bookkeeping clothes.** The entry identifier is bound into the DEK-wrapping nonce and into each chunk's associated data (§3.2, §3.3). Deriving the next identifier from the highest *live* entry — the obvious implementation — reissues a deleted entry's identifier the moment the highest entry is removed, and a wrapped key from the dead entry then decrypts under a live one's nonce. The counter must outlive the entries it counted, so it lives in the document. Deleting every entry does not reset it.

The entry carries no absolute source path. The original Veil stored one, which retained a fact about the user's machine that nothing needed.

### 4.4 Atomic index persistence

Two slots, `index.a` and `index.b`, each self-authenticating and carrying its generation number. A write serialises the whole index, encrypts it, writes it to **the slot holding the older generation**, and fsyncs. A read takes the slot with the highest generation that authenticates.

No rename is involved, because rename atomicity varies across platforms and filesystems while "the older slot is expendable" holds everywhere. A crash mid-write damages only the expendable slot, and the previous generation remains intact and openable (HC-4).

Rewriting the whole index on every mutation is accepted deliberately: at C-1 it costs tens of milliseconds, and it makes the durability argument a single sentence instead of a journal replay implementation. If C-1 ever rises by an order of magnitude this becomes an append-log with periodic checkpoints; the format version field is the door.

### 4.5 Pack files and space management

Packs are append-only, capped at **1 GiB** (initial; tunable). The cap is a **value the API accepts**, defaulting to that figure, not a compile-time constant: a test that must write a gigabyte to reach the spanning path or S-4's attribution gets marked ignored within a month, and those are among the most consequential properties in the format. The same holds for C-1 and C-2 — the entry and file-size limits are values with those defaults, because FR-15's requirement is that the refusal *names both numbers*, and a refusal only reachable by writing 64 GiB is a refusal nobody has watched fire.

The cap is what satisfies three requirements at once:

- **S-3** — adding a file dirties one pack plus the index, so incremental backup and file-sync transfer bytes proportional to the change, not to the vault.
- **S-4** — a damaged region costs only the entries with extents in that pack. Because extents map packs to entries, the affected entries are enumerable, which is the attribution S-4 requires.
- **FR-25** — compaction rewrites one pack at a time and therefore needs about one pack of working space regardless of vault size.

An entry larger than the cap spans packs through its extent list; at C-2's 64 GiB maximum an entry holds at most 64 extents. Reads seek directly to an extent, so one entry is readable without touching unrelated data (A-5) — the door held open for the mount deferral.

**Deletion** (FR-21) removes the entry from the index and adds its extents' length to `reclaimable_bytes`. The bytes remain until compaction, which is why the Design Guideline requires that deletion say so.

**Compaction** (FR-23, FR-24) selects the pack with the highest garbage ratio, copies its live extents into a new pack **allocated from `next_pack_id`**, fsyncs both the pack and its containing directory (§4.7), updates the index in one generation step, then removes the old pack. Every pack holding any garbage is rewritten, with no minimum ratio: Design §8.4 puts the reclaimable figure inside the button the user presses, and a threshold would make that number approximately true. Stored bytes are copied verbatim — no decryption, no re-encryption, no reissued nonce, no new entry identifier — so moving an extent between packs cannot introduce a cryptographic fault. A pack that cannot be read in full is refused rather than rewritten, naming the entries it costs: rewriting it would copy the damage forward, delete the evidence, and report success (HC-3, S-4). The vault is openable at every point, and the loss from interruption is bounded by one pack of copying (HC-4).

**Nothing reconciles packs against the index when a vault is opened, and this section previously said the opposite.** Opening reads the header and one index slot. It writes nothing and it walks no directory. Requirements v2.0 withdrew FR-32, which had required otherwise, and the two reasons are both properties of this format rather than of the product:

- *Unreferenced bytes are only probably leftovers.* An index that is behind its packs is byte-for-byte indistinguishable from an interrupted ingest, and a sync daemon delivering an older index before the packs a newer one describes produces exactly that. Acting on the guess destroys content the newer index still names.
- *A write at open advances `generation`, and `generation` is FR-27's entire mechanism* (§4.4). A vault opened from a stale copy would come away holding a number above the newer index arriving moments later, and every subsequent write would pass the check meant to refuse it. Found by two Phase 2 cases failing the moment reconciliation committed.

**Where the space is found instead.** Both operations that need it already walk the packs, and both are things a user asked for:

- **Reclaiming** (§5.1) measures the packs before selecting anything, so a pack holding bytes no commit accounted for is a candidate like any other and is swept with the rest. This is also where the statistics are trued up against the filesystem, in the same generation step that moves the extents.
- **Reporting the figures** measures them without writing, which is what `recount_statistics` is for (§5.1).

Telling leftovers from a deleted file's bytes needs no new field: the statistics count what committed operations put on disk, the filesystem counts what is there, and the difference is exactly what no commit accounted for. Both are recovered by the same act and FR-8 counts them as one figure, so the distinction never reaches a user.

**The incremental figures can therefore understate, and that is accepted deliberately.** After an interruption, `physical_bytes` and `reclaimable_bytes` are lower than the filesystem's truth until something measures. The alternative — measuring at open — costs one `stat` per pack, and pack count follows vault size, so it puts vault size into the cost of every open and fails S-2. Understating reclaimable space is safe in the only direction that matters: the user is promised less than they get.

On read-only storage — mounted image, write-protected drive, permissions that deny writing — the vault opens read-only and says so at open (FR-26). Refusing would make an interrupted compaction on a drive that later became read-only into permanent data loss, which HC-4 forbids.

A pack file that is *missing* while the index still references it is treated as total damage to that pack, not as a broken vault: the vault opens, the entries with extents in it are enumerated and reported unreadable, and every other entry stays retrievable (S-4). Refusing to open would convert the loss of one pack into the loss of the whole vault, which is the failure mode S-4 exists to reject.

**Damage and unused space are never confused, and the statistics are never adjusted to match damage.** A missing pack is *referenced* by definition, so it is never a candidate for reclaiming and never counted as space to recover. Where a measurement finds more on disk than the index accounts for, the totals are trued up; where it finds *less*, they are left exactly as they are and the shortfall is reported as damage. Quietly writing a smaller vault into the index because a pack vanished is damage covering its own tracks.

### 4.6 Name normalisation

*Resolves the Requirements open question on filename normalisation.* HC-8 makes this defect-grade rather than cosmetic.

- **Stored form: Unicode NFC, UTF-8.** Names are normalised on ingest. macOS presents NFD from its filesystem APIs and Linux stores whatever bytes it was given; without normalisation two visually identical names differ in bytes depending on where the vault was written, which breaks both FR-13 matching and HC-8.
- **Comparison is exact and case-sensitive** after normalisation. The vault is the authority, not the host filesystem — case-insensitive matching would make a vault's contents depend on which machine last touched it.
- **Identity is the full path**: the `folder` field and `name` together (FR-13). Two entries sharing a name in different folders are unrelated, and a replace targets exactly one of them. Matching on name alone would let an ingest into one folder silently overwrite a file in another.
- **Path separators are not stored in names.** The `folder` field holds `/`-separated segments regardless of platform; the separator is a serialisation detail, never the host's.
- **Extraction reconciles with platform limits rather than silently mangling.** A name that is legal in a vault but not on the extracting platform — reserved device names or characters on Windows, a case collision on a case-insensitive filesystem — stops and asks for an alternative. Silent rewriting would produce a file whose name does not match the vault's, which is the confusion HC-8 exists to prevent. See §11.2, feedback item 1.

---

### 4.7 Ingest and extraction

**Ingest is a copy** (FR-9). The source is opened read-only and is never modified, moved, or unlinked. Nothing in `veil-core` deletes a file outside a vault.

**Folder ingest walks regular files only** (FR-10). Symbolic links are not followed and are recorded as skipped (FR-11); following them risks cycles and captures data outside the tree the user selected. Each file's path relative to the added root becomes its `folder` metadata, normalised per §4.6.

**Both directions stream** (A-2, S-1, FR-20). The source is read in chunk-sized reads, each chunk encrypted and appended to the open pack; extraction seeks to each extent and writes decrypted chunks to the caller's `Write`. Neither direction holds more than a small constant number of chunks, so peak memory is independent of file size at C-2's 64 GiB maximum. BLAKE3 is computed in the same pass, so hashing costs no extra read.

**Ordering is what makes FR-12 true.** Pack data is written and fsynced *before* the index generation advances and is fsynced. A crash between the two leaves pack bytes that no index references — swept by the next reclaim (§4.5) — and never an index entry pointing at bytes that were not durable. Success is reported only after the index fsync returns.

**A file's name is durable only when its directory is.** `fsync` on a file makes that file's *contents* durable; the directory entry that gives those contents a name lives in the parent directory and needs its own sync. Without it a crash can leave a fully-synced pack that nothing can find, or a header renamed on one machine and not on another — a durable file with an undurable name, which fails FR-12 while every fsync in the code returned successfully. **The containing directory is therefore inside the same ordering obligation as the file**, and is synced after any operation that creates, renames over, or removes a file within it: a newly created pack, the header's initial write, an index slot's first creation, and the removal that follows a reclaim. This document did not say so through three phases, which is how it came to be missing from all four paths; it is stated here rather than left to the discipline of whoever writes the next one.

**Replace** (FR-13) writes the new entry to completion and durability first, then advances one index generation that simultaneously points the name at the new entry and marks the old one's extents reclaimable. There is no window in which zero intact versions exist.

**Cancellation** (FR-14) is checked at chunk boundaries. Because the index has not advanced, cancelling an ingest leaves only unreferenced pack bytes, and the vault is indistinguishable from one where the operation never started — which is precisely what the Design Guideline promises the user when it says so.

**Extraction verifies before it succeeds** (FR-16, FR-17). Chunk authentication fails fast on tampering, and the BLAKE3 hash is compared against the index after the final chunk. On either failure the partial output is removed and the error names the affected entry.

### 4.8 Verification

Verification (FR-33) reuses the extraction path of §4.7 with the output discarded: every entry's chunks are decrypted and authenticated in order, and the BLAKE3 hash is compared against the index. Nothing is written, so verification runs on a read-only vault and takes no more than a shared lock.

Failure is per entry, not per vault. A failing entry is recorded and verification continues, so one damaged pack yields a complete list of what it cost rather than stopping at the first casualty — which is the attribution S-4 requires and what §8.6 of the Design Guideline presents.

Progress is reported per entry rather than per byte, because the Design Guideline's estimate is in time and entry counts are what a user can hold in their head. Cancellation returns the entries verified so far and their results; a partial verification is a partial answer, not a discarded one.

Verification reads the entire vault and is therefore never scheduled, never automatic, and never triggered at open (FR-33, FR-23).

---

## 5. Application Layer

### 5.1 Core API

The shape `veil-core` exposes, and the constraints it satisfies. Signatures are illustrative.

```rust
Vault::create(path, password, params) -> Result<Vault>      // FR-1
Vault::open(path, password) -> Result<Vault>                // FR-2, FR-5, FR-30
Vault::change_password(&mut self, old, new) -> Result<()>   // FR-4, rewraps MK only
Vault::lock(self)                                            // FR-3, zeroises

Vault::entries(&self) -> &[Entry]                            // FR-6, from memory
Vault::find(&self, folder, name) -> Option<&Entry>           // FR-13, full-path identity (§4.6)
Vault::statistics(&self) -> Statistics                       // FR-8, FR-22
Vault::access(&self) -> Access                               // FR-26, read-only stated at open
Vault::limits(&self) / set_limits(&mut self, Limits)         // C-1, C-2, FR-15 (§4.5, §11.1)

Vault::recount_statistics(&self) -> Result<Statistics>       // FR-8, measured from disk on request
Vault::missing_packs(&self) -> Vec<PackId>                   // S-4, §4.5
Vault::unreadable_entries(&self) -> Vec<EntryId>             // S-4, the attribution, no content read

Vault::add(&mut self, src, folder, &mut dyn Progress, &Cancel) -> Result<EntryId>
Vault::replace(&mut self, id, src, …) -> Result<EntryId>     // FR-13
Vault::extract(&self, id, dst: &mut dyn Write, …) -> Result<()>
Vault::extract_to_path(&self, id, path, …) -> Result<()>     // FR-17, removes partial output
Vault::delete(&mut self, id) -> Result<()>                   // FR-21
Vault::reload(&mut self) -> Result<()>                       // FR-27, adopts an external change
Vault::compact(&mut self, &mut dyn Progress, &Cancel) -> Result<Reclaimed>
Vault::verify(&self, &mut dyn Progress, &Cancel) -> Result<VerifyReport>  // FR-33, §4.8
```

`extract` writes to a `Write` rather than returning bytes, which is what makes S-1 structural rather than a discipline the caller must maintain. It therefore never learns a path — and FR-17's "the incomplete output is removed" can only be honoured by whoever created the file. `extract_to_path` is that owner, and it lives here rather than in each frontend: writing the removal twice is how two frontends come to differ, which A-4 forbids.

**`statistics` and `recount_statistics` are two different promises and the split is the point.** `statistics` is a field read: immediate at any vault size, counting what committed operations put on disk, and what FR-22 requires to be available the moment a vault opens. `recount_statistics` measures the packs: its cost follows vault size, it is never called by `open`, and it is the only source of what is *actually* on disk. A frontend showing figures at open uses the first; a frontend reporting figures because the user asked for them uses the second; reclaiming uses the second and then commits it.

**`missing_packs` and `unreadable_entries` are S-4's attribution and are computed on call, never at open.** One existence check per referenced pack, no content read, so a vault with a missing pack opens at the same speed as one without and the casualties are enumerable the moment anything asks.

`reload` is the second half of FR-27. Detecting an external change and refusing to write over it is only useful if there is a way forward; requiring the password again to get one would make the safe answer cost more than the unsafe one, which is how a safety mechanism becomes something users route around. The subkeys are already held, so a reload is one index read.

 The whole index is resident once opened, so browsing is memory-speed (FR-6) and statistics are a field read rather than a scan (FR-22).

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
| 4 | `NotAVault`, `FormatTooNew`, `FormatSuperseded` | FR-5, FR-30 |
| 5 | `Corrupt` — damage found, including by verification | HC-3, FR-33, S-4 |
| 6 | `VaultInUse` | FR-26 |
| 7 | `ChangedOnDisk` | FR-27 |
| 8 | `ReadOnly` | §4.5, §4.8 |
| 9 | `LimitExceeded` | FR-15 |
| 10 | `Cancelled` | FR-14, FR-19 |
| 11 | `StorageUnavailable` | FR-28 |
| 12 | A required password was not supplied and no terminal is available to ask | §5.2 above |
| 13 | `NotFound`, `AlreadyExists` | FR-34 |

**Compaction is not exposed as a schedulable operation** (FR-23, resolving that Requirements open question). No timer flag, no daemon mode, no "run if reclaimable exceeds N" switch. FR-23 forbids automatic compaction, and a scheduling hook would be that prohibition defeated under another name — a user who wires it into cron has automatic compaction whatever the manual page calls it. Verification carries no such restriction: it only reads, so running it from a scheduled backup script is legitimate and supported.

### 5.3 Graphical application

**Tauri v2.** Rust backend linking `veil-core` directly, web frontend in the system webview.

The alternative considered was egui, which would have removed the webview and with it the persistence risk handled below. It was rejected on evidence: egui's text layout lacks complex-script shaping, and the known workarounds — pre-rendering to a texture, or wrapping a webview — either reimplement the problem or reintroduce the dependency. Veil2's entire interface is user-supplied filenames rendered under HC-8, so a toolkit that cannot render Thai, Hindi, Arabic, or other complex scripts correctly is disqualified regardless of its other merits. Correct rendering of the user's own filenames is not a refinement here; it is the product working.

Vault operations run on a worker thread and report progress to the UI thread through Tauri's event channel, satisfying A-3 without blocking the webview.

**Webview persistence is a defect risk against HC-1 and is closed explicitly.** A system webview will, by default, persist caches, `localStorage`, and IndexedDB to disk. Veil2 renders decrypted filenames into that webview, so anything it persists is an index disclosure. Each platform is configured for ephemeral storage, and none of it is left to default:

| Platform | Obligation |
|---|---|
| macOS | `WKWebView` with a non-persistent `WKWebsiteDataStore` |
| Windows | WebView2 with its user-data folder placed under a per-session temporary directory, removed on exit |
| Linux | WebKitGTK with an ephemeral `WebContext` |

Reinforcing rules, each testable: the frontend uses no `localStorage`, `sessionStorage`, or IndexedDB at all; Content-Security-Policy is restricted to the bundled origin with no remote host permitted, so no vault-derived string can leave the machine; developer tools are compiled out of release builds. §9 carries the test that verifies this rather than trusting it.

**All three configurations are written; each is verified on the platform it ships with, and not before** (§8.1, Requirements §2.1). The persistence test of §9 is a release gate per platform rather than a 2.0 gate, and this is the one place where the platform deferral has a security consequence rather than a portability one: these configurations fail *silently* when wrong. An unverified webview configuration is an unverified HC-1 claim, so **a platform does not ship until its persistence test has been run on it** — the deferral moves the run, never the gate.

**Accepted cost.** Tauri brings a JavaScript toolchain and its dependency tree into a security product's supply chain, which egui would not have. It is bounded by the same policy as §7: pinned versions, audited by the same gates, and no frontend dependency permitted to make network requests at runtime.

---

## 6. Error Handling

Typed errors via `thiserror` throughout `veil-core`. **`anyhow` is not used in the library** — the original Veil converted every failure into a single string-carrying variant, which is why a wrong password and a corrupted vault were indistinguishable to callers. Binaries may use `anyhow` at their top level.

The taxonomy distinguishes, at minimum:

| Variant | Satisfies |
|---|---|
| `WrongPassword` | FR-2 — distinct from corruption, so the user is sent to the right remedy |
| `FormatTooNew { required, supported }` | FR-5 |
| `FormatSuperseded { version }` | FR-30 |
| `Corrupt { what, affected_entries }` | HC-3, S-4 — carries which entries are affected |
| `VaultInUse`, `ChangedOnDisk`, `StorageUnavailable` | FR-26, FR-27, FR-28 |
| `LimitExceeded { limit, value }` | FR-15 — carries both numbers the message must name |
| `Cancelled { rolled_back: bool }` | FR-14, FR-19 — states what the cancel left behind |
| `VerificationFailed { entries }` | FR-33, S-4 — carries every failing entry, not just the first |
| `ReadOnly` | §4.5, §4.8 — the vault opened without a lock because its storage would not take one, and a write was attempted |
| `NotFound` | FR-2's principle applied to naming: a path that matches nothing is a mistyped name, not damage |
| `AlreadyExists` | FR-34 — the path is already a file's identity, and adding a second is refused |
| `PasswordTooShort { minimum }` | FR-1, C-4 — raised where a password is set, never where one is offered to open a vault |

Two prohibitions, each with its reason:

- **No error, `Display`, or `Debug` output contains plaintext, file content, key material, or the password** (HC-2). Key types have hand-written `Debug` implementations that print a placeholder.
- **Logging never records entry names, folder metadata, or content** (HC-1). `tracing` is used for operational events only — operation started, bytes processed, error variant. A log file that reconstructs the index would defeat the vault.

`NotFound` is its own variant for the same reason `WrongPassword` is: a mistyped name and a damaged vault send a user to entirely different remedies, and reporting the first as the second is the original Veil's defining failure repeated at the level of names. It was reported as `Corrupt` with an empty affected list until Phase 3 found it.

Neither `NotFound` nor `AlreadyExists` carries the path, for the reason the I/O variant carries none: the caller supplied it and is the layer that can name it. It is also the only layer permitted to — a file name is index data, and HC-1 is why no variant here holds one.

`ReadOnly` is its own variant rather than an I/O error carrying a read-only kind. Both §4.5 and §4.8 require a read-only vault to *open* — refusing would turn an interrupted compaction on a drive that later became write-protected into permanent data loss, and would make the operation that diagnoses a failing drive the one operation a failing drive cannot run. So the refusal happens at the write, and it is a condition the frontends must phrase differently from a disk failure: nothing is wrong, the vault simply cannot be changed from here.

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
| `blake3` | Content hashing | FR-17 |
| `zeroize` | Key material lifetime | §3.1 |
| `ciborium`, `serde` | Index serialisation | §4.3, FR-30 |
| `rand`, `getrandom` | CSPRNG for keys, salts, nonces | §3.1 |
| `fs4` | Cross-platform advisory locks | FR-26 |
| `thiserror` | Error taxonomy | §6 |
| `tracing` | Operational logging, subject to §6 | |
| `clap` | CLI argument parsing | A-4 |
| `anyhow` | Error handling at a binary's top level only. §6 forbids it in the library and that prohibition stands — the reason it exists is that the original Veil made a wrong password and a damaged vault indistinguishable, which is a property of the library's taxonomy, not of how a binary prints one | §6 |
| `serde_json` | Machine-readable command-line output | Design §3.4 |
| `ctrlc` | Interrupt handling, so an interrupt reaches the cancellation the core already implements rather than killing the process | FR-14, FR-19 |
| `rpassword` | Reading a password from a terminal without echoing it | HC-2 |
| `tauri` (v2) | GUI shell, webview integration, native dialogs | §5.3 |

The frontend toolchain is pinned and lockfile-committed like the Rust dependencies, and audited by the same gates. No frontend dependency may make a network request at runtime; the Content-Security-Policy of §5.3 enforces this rather than relying on review.

**The table above covers dependencies that ship.** Test-only dependencies are held to the same pinning and audit policy but are listed separately, because a crate that cannot reach a release binary is a different risk: `proptest` (property tests, §9), `assert_cmd` (CLI tests, §9), `tracing-subscriber` (the logging guard of §6), and `serde_json` in `veil-core`'s tests only, where it is the readable form for fixtures and never a serialisation path the library uses — §4.3 fixes CBOR for that. Adding one still requires a bump of this section, so the set stays deliberate.

The original Veil's `sled` is not carried forward: it has been unmaintained since 2021, and §4.3 and §4.4 do the job in a few hundred lines with a durability story that fits in a paragraph.

---

## 8. Build and Release

### 8.1 Toolchain

Rust 2024 edition. Release versions begin at 2.0.0 per Requirements §8.

No CI pipeline. The gates run locally before every commit: `cargo fmt --check`, `cargo clippy --workspace --all-targets` at `-D warnings`, `cargo test`, `cargo deny check`, `cargo audit`. All must pass.

**macOS is what 2.0 ships and what everything here is run on. Windows and Linux are built for, unverified, and released separately** (Requirements §2.1, §2.2). This replaces the earlier "should work but are unconfirmed", which described the same situation while the surrounding documents still called the three platforms peers.

The obligation this puts on the code is unchanged and is the whole reason the deferral is affordable: nothing in the language, the dependencies, or the format is macOS-specific, and every path that differs by platform — advisory locks, directory durability, symbolic links, file permissions, path handling, name normalisation — is written against the standard library's cross-platform APIs and reviewed as if all three ran tomorrow. **Writing for one platform is forbidden; verifying on one is what is accepted.** The distinction is the entire content of this section:

- A defect that is *platform-specific behaviour reaching the stored format* is an HC-8 violation and is defect-grade today, on a machine nobody has yet run it on. It is caught by review and by keeping the format free of host facts (§4.6), not by having three machines.
- A defect that is *this build has not been executed on that OS* is what the deferral accepts, and it is bounded: it costs a release date, not a vault.

What closes it is a machine and a test run, per platform, as §9 and Requirements §8 describe.

### 8.2 Packaging

**Ships with 2.0:**

- **macOS** — `.veil` registered as a document bundle via a declared UTI, so the vault reads as a single document in Finder (§4.1). Application signed and notarised.

**Designed, unbuilt, and released with their platforms** (§8.1). Each is recorded now rather than later because both carry design consequences that reach back into decisions already made, and discovering either after 2.0 would be discovering it too late:

- **Windows** — installer registers the `.veil` folder association. No bundle equivalent exists; the vault presents as a folder, so §4.1's single-document feel is a macOS property and not a portable one.
- **Linux** — AppImage as the primary artifact for distribution independence, with a desktop entry and MIME association. Tauri's Linux webview is WebKitGTK, which is a system library rather than a bundled one, so the AppImage declares its minimum version and the application fails at startup with a clear message when it is absent or too old. This is the concrete cost of §5.3's toolkit choice and it lands entirely on Linux packaging.

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
| Corrupt one pack | Fail **only** the entries with extents in it, and name them (S-4) |

**Crash tests for HC-4:** kill the process mid-operation during `add`, `replace`, `delete`, and `compact`, then assert the vault opens, the index authenticates, statistics match a full recount, and no entry that existed beforehand is lost. A real kill, not a simulated one — an abstraction inside `veil-core` that let a test pretend to crash would add a seam to shipped code to serve a test, and would be testing the pretence. The kill is a kill and not an interrupt: an interrupt is cancellation, which is a different guarantee with its own tests (FR-14).

**What is killed:** the shipped `veil` binary for `add`, `replace` and `delete`, and a test-only subject binary linking `veil-core` for `compact`. Compaction lives in multi-pack behaviour, the pack cap is a value the API accepts precisely so a test can reach that behaviour without writing gigabytes (§4.5), and the command line does not expose the cap and must not — a flag whose only purpose is to make a test cheap is the seam the paragraph above refuses. Both subjects are real processes killed for real, which is the property that matters; neither is simulated and the second reaches no release.

*What the crash tests cannot reach, stated rather than implied:* killing a process does not empty the operating system's page cache. A vault surviving every kill here has proved the **ordering** holds — that no index generation names bytes the code had not yet synced — and has not proved that the platform's `fsync` reaches the platter. Whole-machine power loss is the only test for that and there is no rig for it (§11.1).

**Cross-platform check for HC-8, and it is a release gate for the platform it is run on** (§8.1, Requirements §2.1): write a vault containing filenames in Latin, Thai, Arabic, Han, and emoji, including NFC/NFD pairs and names reserved on Windows, then open it on each other platform and compare names and content byte-for-byte. Run by hand, per platform, as that platform approaches release.

What is built before then, on macOS, is the **fixture** — the vaults themselves, generated and committed, together with the comparison the check performs. That much needs one machine and is what makes the eventual run an afternoon rather than a project, and it is also where a normalisation defect is most likely to be caught: a vault whose stored names are wrong is wrong on the machine that wrote it, and the NFC/NFD pairs are checkable there. What genuinely needs three machines is narrower than it looks — that the other two open what macOS wrote and present the same names.

**The header and index parsers get randomised input** — they are the only attacker-controlled inputs reachable before authentication, so a panic in either is a defect regardless of who reaches it. `cargo-fuzz` would be the right tool and is declined (§11.1): it needs a nightly toolchain and a tool on the machine. What runs instead is seeded and deterministic, so a failure reproduces exactly, and it covers the same two entry points at lower depth.

**Webview persistence test for §5.3, on every platform that ships.** Open a vault whose entries carry distinctive marker filenames, browse them, close the application, then search the webview's data directory, the application's cache directories, and the system temporary directory for those markers. Any hit is a defect against HC-1. This test exists because §5.3's three platform configurations fail silently when wrong — nothing about the running application looks different if a cache is being written — and a silent HC-1 violation is the failure mode this project was rebuilt to eliminate. It is the one test the platform deferral does not relax: **no platform ships without it, because an unverified webview configuration is an unverified HC-1 claim** (§5.3).

**Scale tests**, marked `#[ignore]` and run on request, since a multi-gigabyte fixture costs minutes and disk: a multi-gigabyte entry, and a vault at C-1's entry limit, asserting S-1 (peak memory does not scale with file size) and S-2 (open time does not scale with vault size).

---

## 10. Milestones

High-level; the Implementation Plan expands each into phases and tasks. Each states what it proves.

**M1 — Format and crypto core.** Header, key hierarchy, STREAM content encryption, pack write and read, index persistence. *Proves the format and the cryptographic construction, and that tampering and truncation fail loudly — the adversarial suite of §9 passes before anything is built on top.*

**M2 — Vault operations.** Add, list, extract, replace, delete over the §5 API with progress and cancellation. *Proves the core API is sufficient for both frontends without either existing yet.*

**M3 — CLI at full parity.** *Proves the core is usable with no UI, and establishes the integration test surface A-4 depends on.*

**M4 — Durability and compaction.** Crash-injection suite, compaction with bounded working space, and the bytes an interrupted operation leaves swept by the next reclaim. *Proves HC-4 and FR-25 — the properties that make a vault trustworthy at hundreds of gigabytes.*

**M5 — Portability by construction.** Everything HC-8 requires that one machine can establish: NFC normalisation on ingest, the representability check at extraction (FR-31), the network-path advisory, the path-metadata limit, and the §9 portability fixture built and committed with the comparison it feeds. *Proves that no host fact reaches the stored format — which is the half of HC-8 that is defect-grade now — and leaves the other two platforms an afternoon of running rather than a project.*

**M6 — GUI foundation.** Tauri shell over `veil-core`, all three ephemeral-webview configurations of §5.3 written and the shipping platform's verified by the §9 persistence test — the other two verified at M8, and neither platform shipping until it is — and the entry list rendering complex-script filenames correctly in both themes with OS drag-and-drop and native dialogs working. *Proves that the webview cannot leak the index, and that the one thing the interface exists to do — display the user's own filenames correctly — actually works before any feature is built on it.*

**M7 — GUI v1, and 2.0.0 for macOS.** The Design Guideline realised, packaged per §8.2. *Proves the product.*

**M8 — Windows, then Linux.** Per platform: build, run the whole suite, run the §9 portability check against the fixture M5 committed, run the §9 webview persistence test, package per §8.2, release. *Proves HC-8 in the direction only a second machine can, and closes the deferral Requirements §2.2 records.* Each platform is its own release with its own version number; neither is announced before its run.

M1 through M5 touch no GUI code, so the interface work blocks nothing and is blocked by nothing until M6. M8 depends on hardware rather than on work, which is why it is last: nothing before it is blocked by a machine that does not exist, and the deferral therefore costs a release date rather than a phase.

---

## 11. Open Items and Feedback

### 11.1 Open items

- **Final Argon2id parameters.** `m = 256 MiB, t = 3, p = 4` is what new vaults are created with. **Measured on the development machine at 0.27 s in a release build** (Apple silicon, macOS; `cargo test -p veil-core --test kdf_cost -- --ignored --nocapture` reproduces it, and prints the neighbouring parameter sets for comparison). That is comfortably inside C-3's one-second budget on fast hardware, which is the wrong end of the range to measure: C-3's target is the *weakest* supported machine, and this one is among the strongest. The item stays open for that reason and that reason only — one number is now known rather than none. Nothing is orphaned by a later change: HC-5 means every vault records what it was created with, and opening reads that and never a constant. Resolver: the same measurement on low-spec hardware when available.
- **Pack size cap.** 1 GiB initial, and a value the API accepts (§4.5). Smaller improves sync granularity (S-3) and damage locality (S-4); larger reduces file count and per-pack overhead. Resolver: tune with use once real vaults exist.
*Resolved in v2.0:*

- **Whether compaction may proceed while a read is in flight.** Resolved: **it may not, and nothing is added to permit it.** §5.1 fixes the operation as taking an exclusive borrow of the vault, so no read through the same vault can be in flight, and a second process is refused by the advisory lock long before this matters. What Design §8.4 promises — that the vault stays usable while space is being reclaimed — is browsing, and browsing is served from the resident index without touching a pack (FR-6). How the graphical application holds the vault while a worker reclaims is an architecture question for M6, not a format one.
- **The fsync ordering of §4.7 — verified to the extent a process kill can verify it, and the residue is stated rather than closed.** M4 killed real processes at the boundaries and the ordering holds: no index generation names bytes the code had not yet synced. What remains unverified is whether the platform's `fsync` reaches the platter, which needs whole-machine power loss and a rig that does not exist (§9). The item is not closed by pretending otherwise; it is closed as *the ordering is proved and the platform's honouring of it is not*, which is a smaller and honest claim. The indirection layer stays rejected.

*Resolved in v1.2:* **`cargo-fuzz` on the header and index parsers (§9)** — declined. It requires a nightly toolchain and a tool installed on the development machine, and the owner has ruled that out. The seeded randomised testing that exists covers the same two entry points at lower depth and runs with the suite; §9's fuzzing line stands as a description of what would be better, not of what is done.

*Resolved during v1.0:* **GUI toolkit** — resolved as Tauri v2, §5.3. egui was the preferred candidate on supply-chain and webview-persistence grounds, and was rejected on evidence: its text layout ranks among the weakest available for complex scripts such as Thai and Devanagari, and the known workarounds are pre-rendering to a texture or wrapping a webview, which respectively reimplement the problem and reintroduce the dependency. Under HC-8 the interface is filenames, so this is disqualifying. The webview persistence risk that egui would have avoided is closed explicitly in §5.3 and verified by the §9 test rather than assumed. Filename normalisation (Requirements open question) — resolved as NFC with exact case-sensitive comparison, §4.6. A referenced pack that is missing entirely (Requirements open question) — resolved as total damage to that pack, opening the vault and reporting the affected entries rather than refusing, §4.5. Withdrawal of support for a superseded format version (Requirements open question) — resolved as **not permitted while the migration path of Requirements §2.2 remains unbuilt**; a release may not refuse a vault it can still read, because there is no other route by which the user's data could be recovered.

### 11.2 Feedback to upstream documents

Recorded per G-24 and permanent. Items 1 to 3 were raised while the suite was unapproved and converging on 1.0, so the owner absorbed them directly. Items 4 to 7 were raised by implementation against an approved suite and absorbed as the reversals recorded in Requirements v2.0.

**1. Extraction may fail on a name the platform cannot represent. — Absorbed as FR-31.** FR-18 covered overwriting an existing destination file, but not a vault name that is legal in the vault and illegal on the extracting platform — Windows reserved names and characters, or a case collision on a case-insensitive filesystem. HC-8 makes the vault's names authoritative, so §4.6 stops and asks rather than silently rewriting, and FR-31 now requires it.

**2. FR-26 needs an honesty clause for network filesystems. — Absorbed into FR-26 and Requirements §7.** Advisory locks are unreliable on NFS, SMB, and some user-space mounts, so on those paths FR-26 is best-effort and FR-27's detection is the real protection (§2). FR-26 now says so, and §7 lists live network sharing among the things Veil2 does not defend against.

Items 4 to 7 were raised by Phase 4 against an approved suite and absorbed into Requirements v2.0, so each names what changed upstream rather than what was proposed.

**7. Three platforms were being claimed and one was being run. — Absorbed into Requirements §2.1, §2.2 and §8.** Every document in the suite called macOS, Windows and Linux peers; every gate had only ever been run on macOS, and P0.4's withdrawal removed the one mechanism that could have changed that. A claim nobody can check is not a standard, it is decoration, and leaving it in place would have put Phase 5 at a gate that could not be passed — the pressure then being to walk past it into GUI work, which is the ordering §10 exists to defend. HC-8 is untouched and still defect-grade; what moved is when it is verified. §8.1 above draws the line the code is held to in the meantime.

**6. FR-26 had no answer for storage that will not take a lock at all. — Absorbed into FR-26.** §2 said the lock is held "for the lifetime of the open vault" and said nothing about being unable to take one for reasons other than contention. Raised by Phase 2 and never absorbed until now; the behaviour has been in the code since Phase 2 and is stated in §2 above.

**5. FR-8's reclaimable figure had to say what it counts. — Absorbed into FR-8 and Design §3.2, §8.4.** Deleted files' bytes and bytes an interrupted operation left are recovered by the same act and shown as one number, and the figure the index carries can understate until something measures. A consequence of item 4, recorded separately because it landed on a different document.

**4. Reconciliation at open was wrong, and reversing its verb was also wrong. — Absorbed as FR-32's withdrawal in Requirements v2.0.** This is the item worth reading twice, because it was resolved incorrectly before it was resolved.

Phase 4 found that FR-32's instruction to *discard* unreferenced bytes at open acts on a guess, and that the guess is wrong in the sync-folder case §1 names as a motivation for the product. It proposed reporting instead. That proposal was absorbed, and it kept the mechanism while changing its verb: something still happened when a vault was opened, and this document still described a walk of the packs directory as part of opening. Only afterwards was the prior question asked — *should opening a vault do this at all?* — and the answer removed the requirement rather than editing it.

Two independent objections, either sufficient: unreferenced bytes cannot be identified without guessing, and anything done at open that writes costs FR-27 its detector while anything done at open that walks costs S-2 its guarantee. Both point at *where* the work was placed, not at what it did. Moved to reclaiming and to reporting the figures — both user-initiated, both already walking the packs — the same work is correct and cheap.

**The process lesson is recorded upstream in Requirements §9** and is more valuable than the decision: this item, item 3 below, and every note this suite has absorbed were resolved by writing down what the implementation did. That is one of two available answers and the other was never reached for.

**3. Opening a vault performs reconciliation not described by any requirement. — Absorbed as FR-32, and that absorption was the mistake.** §4.5 removed packs that no index extent referenced, which is how an interrupted compaction or ingest was to be cleaned up under HC-4. The observation was exactly right and is quoted here in the form it was written: *"This is a write occurring during what a user understands as merely opening a vault."*

The resolution was to add a requirement authorising it. Four phases were then built on FR-32 before implementation showed that the write is unsafe (item 4) and that opening a vault must not do this at all. **The smell was detected during design and legitimised instead of investigated** — see item 4, and Requirements §9 for the rule that now applies to notes of this shape.
