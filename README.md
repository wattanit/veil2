<p align="center">
  <img src="crates/veil-gui/icons/128x128@2x.png" width="128" alt="Veil2 icon">
</p>

# Veil2

A file-level encrypted vault for macOS. Files go in individually encrypted; nothing about them — name, size, timestamp, or content — is readable without the password.

Veil2 is not a mounted encrypted volume. A vault is a `.veil` bundle you create, open, and browse as a list; retrieving one file never requires decrypting the rest.

## Why file-level, not a mounted volume

- **Selective access.** A vault can hold hundreds of gigabytes; opening it never means decrypting all of it. Each file is its own encrypted unit.
- **Full control.** Only published, reviewed cryptography is used (Argon2id, XChaCha20-Poly1305, BLAKE3, HKDF). The on-disk format is documented and can be inspected independently of this application. There is no recovery mechanism, no key escrow, and no dependency outside this repository.

**There is no password recovery.** Forgetting a vault's password loses everything in it, permanently. This is a property of the key hierarchy, not a policy choice layered on top of it — see [`docs/Requirements.md`](docs/Requirements.md) (HC-7).

## Status

Version 2.0.1. Builds for, ships on, and makes claims about **macOS only** — Windows and Linux are not built and not scheduled. One shared core library drives both a command-line application and a graphical application.

## Repository layout

| Path | What it is |
|---|---|
| `crates/veil-core` | The vault format, cryptography, and every operation (create, add, save a copy, replace, delete, check for damage, change password). No UI, no `unsafe`, no interactive dependency. |
| `crates/veil-cli` | `veil` — the command-line application. |
| `crates/veil-gui` | The Tauri-based desktop application, and its frontend (`crates/veil-gui/ui`, vanilla TypeScript). |
| `docs/` | Requirements, design, and technical specification — see [Documentation](#documentation) below. |

## Building

### Prerequisites

- A recent stable Rust toolchain (2024 edition).
- Node.js and npm, for `crates/veil-gui/ui`'s frontend build.
- The Tauri CLI, for the GUI only: `cargo install tauri-cli` (once, globally).

### The core library and the CLI

```sh
cargo build --release -p veil-cli
target/release/veil --help
```

### The GUI

```sh
cd crates/veil-gui/ui && npm install && npm run build
cd ../.. && cd crates/veil-gui
cargo tauri dev      # development, with live reload
cargo tauri build    # a release .app / .dmg
```

### Signing and notarizing a release build

`cargo tauri build` produces an unsigned app unless it's told which certificate to sign with. That identity is deliberately kept out of `tauri.conf.json` — it's a real name, not something to commit — and instead lives in a local, gitignored file Tauri merges in automatically on macOS:

```sh
cp crates/veil-gui/tauri.macos.conf.json.example crates/veil-gui/tauri.macos.conf.json
```

Edit that copy's `signingIdentity` to match a certificate already in your keychain — list what's available with:

```sh
security find-identity -v -p codesigning
```

and use the exact string after the quotation marks, e.g. `Developer ID Application: Your Name (TEAMID1234)`.

**Notarization** goes through environment variables instead, at build time, so it never touches any file at all:

```sh
export APPLE_ID="you@example.com"
export APPLE_PASSWORD="an app-specific password"   # not your Apple ID password — see below
export APPLE_TEAM_ID="TEAMID1234"
cd crates/veil-gui && cargo tauri build
```

With all three set, `cargo tauri build` signs, submits for notarization, and staples the ticket in one step — no separate manual submission.

The app-specific password is generated at **appleid.apple.com** → *Sign-In and Security* → *App-Specific Passwords*. Apple requires this because two-factor accounts can't hand their normal password to a command-line tool; the app-specific password is scoped to this one purpose and can be revoked independently of the real password.

## Using the CLI

```
veil create MyVault.veil
veil add MyVault.veil ~/Documents/report.pdf --folder work
veil list MyVault.veil
veil save-copy MyVault.veil work/report.pdf --to ~/Desktop/report.pdf
veil replace MyVault.veil work/report.pdf --from ~/Documents/report-v2.pdf
veil delete MyVault.veil work/report.pdf
veil check MyVault.veil
veil info MyVault.veil
veil password MyVault.veil
```

A password is never taken as a command-line argument — arguments are visible in process listings and shell history. Every command reads the vault's password via `--password-file`, the `VEIL_PASSWORD` environment variable, or an interactive prompt. Run `veil --help` or `veil <command> --help` for the full option list and exit codes.

## The cryptographic construction, briefly

- A password, run through Argon2id, unwraps a random 32-byte master key — the master key is never derived from the password, so a password change rewrites 32 bytes rather than re-encrypting the vault.
- Two subkeys (index, per-entry wrapping) come from the master key via HKDF-SHA256, with distinct domains — no key is reused across purposes.
- Each file gets its own random data key and is encrypted with `StreamBE32<XChaCha20Poly1305>` (the STREAM construction), so a truncated file fails authentication instead of silently decrypting short. A BLAKE3 hash of the plaintext is stored alongside for end-to-end verification.

Full detail, including what an unopened vault still discloses (its total size and file count) and what this project explicitly does not attempt (secure erasure of originals, hidden volumes, defeating traffic analysis), is in [`docs/Requirements.md`](docs/Requirements.md) and [`docs/TechnicalSpecification.md`](docs/TechnicalSpecification.md).

## Testing and gates

All of the following must pass before a commit:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test
cargo deny check
cargo audit
(cd crates/veil-gui/ui && npm audit)
```

There is no CI pipeline; these run locally.

## Documentation

- [`docs/Requirements.md`](docs/Requirements.md) — what Veil2 must do, and why. Start here.
- [`docs/DesignGuideline.md`](docs/DesignGuideline.md) — visual language, interaction policy, and the fixed vocabulary both applications use.
- [`docs/TechnicalSpecification.md`](docs/TechnicalSpecification.md) — the format, the cryptographic construction, and the build.
- [`docs/v2.0/IMPLEMENTATION_PLAN.md`](docs/v2.0/IMPLEMENTATION_PLAN.md) — how the 2.0.0 work was sequenced into phases.
- [`docs/v2.0/implementation/`](docs/v2.0/implementation/) — per-phase to-do lists and test cases for 2.0.0, the record of what was actually verified.
