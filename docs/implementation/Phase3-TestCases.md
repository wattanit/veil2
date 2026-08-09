# Veil2 — Phase 3 Test Cases: Command-Line Application

**Version:** 1.0
**Status:** approved
**Date:** 2026-08-09
**Owner:** wattanit
**Foundation and plan versions these cases are built against:**
- Requirements Document v1.0 — upstream
- Design Guideline v1.0 — upstream
- Technical Specification v1.0 — upstream
- Implementation Plan v1.0 — upstream
- [Phase3-ToDo.md](Phase3-ToDo.md) v1.0 — companion; each case names the item it covers

This document owns the enumerated checks that close Phase 3. Every case cites the requirement it verifies.

**This document supersedes the previous Phase 3 test cases entirely.**

---

## Conventions

**Case identifiers** are `T<phase>.<n>`, sequential within this document.

**Every case runs the built binary as a subprocess.**

**Every case asserts the exit code**, not merely that the command failed.

**Nothing here reads a terminal.**

**Where these run.** The development machine, macOS.

**How to run them.** Every case spawns the real binary, which derives a real key at C-3's cost — the cheap test parameters are compiled out of anything but a debug build.

```bash
cargo test --release -p veil-cli
```

---

## The command surface

### T3.1 — Every core capability is reachable from the shell
*Covers P3.1.a, P3.1.e · Verifies A-4, FR-1 through FR-22, FR-26*

Run a full lifecycle using nothing but the binary: create a vault, add a file, add a folder, list, save a copy out, replace a file, check for damage, read the statistics, change the password, reopen under the new password, delete a file.
**Verdict:** every step exits 0 and the vault is intact at the end.

### T3.2 — No command accepts a password as an argument
*Covers P3.4.b · Verifies HC-2, Spec §5.2*

Pass `--password secret` to every subcommand.
**Verdict:** the parser rejects it on all of them, as an unknown argument.

### T3.3 — Nothing schedules or conditions an operation
*Covers P3.1.g · Verifies FR-23, Spec §5.2*

Search the full help output of every command for a flag that schedules, times, daemonises, or triggers on a threshold.
**Verdict:** none exists.

### T3.4 — A path the vault already holds is refused
*Covers P3.1.d · Verifies FR-14, FR-13, HC-4*

Add a file, then add a second file at the same folder and name.
**Verdict:** the second add exits with the already-exists code, names the path, and points at `replace`. The vault still holds one file at that path, with its original content.

### T3.5 — A path matching nothing is not reported as damage
*Covers P3.1.c · Verifies FR-2, HC-3*

Ask for a file that was never added.
**Verdict:** a message saying no file has that path, and an exit code distinct from both the damage code and the wrong-password code.

### T3.6 — A folder add reports what it skipped
*Covers P3.1.e · Verifies FR-10, FR-11*

Add a folder containing regular files, a symbolic link to a file outside it, and a nested folder.
**Verdict:** exit 0, every regular file stored with its relative folder recorded, and the symbolic link named in the output as skipped.

---

## Checking for damage

### T3.7 — Damage is found, named, and exits non-zero
*Covers P3.1a.a, P3.1a.b, P3.1a.c, P3.1a.d · Verifies FR-26, S-3, Spec §5.2*

Build a vault of several files, check it, then corrupt the stored bytes of two of them and check again.
**Verdict:** the first run exits 0. The second exits with the damage code, names both files, and does not stop at the first. The output states plainly that Veil2 cannot recover them.

---

## Output

### T3.8 — The table is in the fixed column order
*Covers P3.2.a, P3.2.b, P3.1.f · Verifies Design §3.4, §7, FR-6, FR-7*

List a vault holding files across several folders, then list with a folder filter and with a name filter.
**Verdict:** columns in the order Design fixes, sizes human-readable, the count exact and unrounded, and each filter returning exactly the matching rows.

### T3.9 — Stored names are printed exactly
*Covers P3.2.c, P3.2.d · Verifies —*

Add files named in Latin, Thai, Arabic, Han and emoji, and list them.
**Verdict:** every name comes back byte-identical to what was stored. Column alignment for double-width scripts is **not** asserted; the limitation is stated rather than hidden.

### T3.10 — Machine output carries the same facts
*Covers P3.3.a, P3.3.d · Verifies Design §3.4*

List and read the statistics in `--format json`, then run a failing command in the same mode.
**Verdict:** the output parses, exact byte counts rather than human-readable sizes, the same set of facts as the table, and the failure is reported in machine form too.

### T3.11 — The streams stay separated
*Covers P3.3.b, P3.3.c · Verifies Design §3.4*

Run a listing with standard output captured to a file and standard error discarded.
**Verdict:** the file parses as valid machine output from first byte to last, with no progress, no banner, and no prose interleaved.

### T3.12 — The reported statistics are the vault's
*Covers P3.2.e · Verifies FR-7*

Add files, delete one, and read the statistics.
**Verdict:** entry count and total size match what the library reports for the same vault. No reclaimable figure is printed.

---

## Password input

### T3.13 — A password file works and is the only file read
*Covers P3.4.a, P3.4.c · Verifies Spec §5.2*

Open a vault with `--password-file`.
**Verdict:** exit 0. No prompt is attempted.

### T3.14 — The environment variable works
*Covers P3.4.a · Verifies Spec §5.2*

Open the same vault with the password in the environment and no file given.
**Verdict:** exit 0, and the file source takes precedence when both are present.

### T3.15 — A non-interactive invocation with no password fails fast
*Covers P3.4.d, P3.8.c · Verifies Design §3.4, Spec §5.2*

Run a command needing a password with no terminal, no file, and no environment variable, under a timeout.
**Verdict:** it exits before the timeout, with the missing-input code, naming which input was missing.

### T3.16 — The password never appears in the process
*Covers P3.4.b · Verifies HC-2*

Run a long operation with the password supplied by file, and read the command line of the running process.
**Verdict:** the password is not in it.

### T3.17 — A wrong password is distinguishable from a damaged vault
*Covers P3.4.g · Verifies FR-2*

Open a good vault with the wrong password. Then damage a vault's header and open it with the right one.
**Verdict:** two different exit codes and two different messages.

### T3.18 — A password file is trimmed exactly once
*Covers P3.4.e · Verifies Spec §5.2*

Create a vault with a password file written by `echo` (one trailing newline). Reopen using a file with no trailing newline, then one with two.
**Verdict:** the first two open; the third does not.

### T3.19 — Creation states unrecoverability before it creates
*Covers P3.1.h, P3.4.f · Verifies HC-7, FR-1, FR-27, C-4*

Create a vault. Then attempt one with a password below the C-4 minimum.
**Verdict:** the output states that a lost password cannot be recovered, before the vault exists. The short password is refused, naming the minimum.

---

## Progress and cancellation

### T3.20 — Progress goes to standard error, results to standard output
*Covers P3.5.a, P3.3.c · Verifies Design §3.4*

Add a file large enough to produce progress, capturing the streams separately.
**Verdict:** progress appears only on standard error; standard output carries only the result.

### T3.21 — Off a terminal, progress is periodic lines
*Covers P3.5.b, P3.5.c, P3.8.c · Verifies Design §3.4*

Run the same add with standard error captured to a file.
**Verdict:** plain lines, no carriage returns and no escape sequences.

### T3.22 — An interrupt cancels rather than kills
*Covers P3.5.d, P3.5.e · Verifies FR-15, FR-19, HC-4*

Start an add of a large file, send an interrupt mid-operation, and wait for the process.
**Verdict:** the cancelled exit code, a message saying what was left behind, and a vault whose contents and statistics are exactly what they were before the command started. Reopening it succeeds.

---

## Exit codes and refusals

### T3.23 — One code per error class
*Covers P3.6.a, P3.6.b, P3.6.c · Verifies Spec §5.2, §6, FR-2*

Provoke each condition in Spec §5.2's table that this phase can reach — success, usage error, wrong password, not a vault, damage, vault in use, read-only, limit exceeded, cancelled, missing password.
**Verdict:** each produces its own code, no two conditions share one, the codes appear in the help output, and no output uses exit code 14.

### T3.24 — A vault already open is reported as in use
*Covers P3.6.a · Verifies FR-23*

Hold a vault open, then run a command against the same directory.
**Verdict:** the in-use code, and a message saying the vault is open.

### T3.25 — A read-only vault reads but does not write
*Covers P3.6.a · Verifies Spec §4.5, §4.8, FR-23*

Make a vault directory read-only. List it, check it, then try to add.
**Verdict:** listing and checking succeed; the add exits with the read-only code.

### T3.26 — A destination file is never overwritten unasked
*Covers P3.1.h · Verifies FR-19*

Save a copy to a path that already holds a file, without a pre-confirmation flag, then with one.
**Verdict:** the first refuses, naming the file it would have overwritten; the second proceeds.

### T3.27 — A failed save leaves no partial file
*Covers P3.1a.a · Verifies FR-17, HC-3*

Corrupt a file's stored bytes and save a copy of it to a fresh path.
**Verdict:** the damage code, and nothing at the destination.

### T3.28 — Adding says the original is still there
*Covers P3.1.h · Verifies FR-9, FR-27*

Add a file.
**Verdict:** the source file exists, unmodified, and the output says so.

### T3.29 — Deleting says the bytes are gone
*Covers P3.1.h · Verifies FR-22, FR-27*

Delete a file.
**Verdict:** it is gone from the listing, and the output states plainly that it has been removed rather than claiming it can be recovered.

### T3.30 — A limit names both numbers
*Covers P3.6.a · Verifies FR-16*

Provoke the per-file limit and read what it says.
**Verdict:** a message carrying both the limit and the actual size. Provoked through the library rather than the binary, since the limit is 64 GiB (C-2) and no flag lowers it.

---

## Audits

### T3.31 — The vocabulary holds across the whole surface
*Covers P3.1.a, P3.8.d · Verifies Design §7*

Collect every command's help text and every message the suite produces, and search for Design §7's forbidden column and forbidden claims.
**Verdict:** none appears, and no command named `reclaim-space` exists.

### T3.32 — No output discloses what it must not
*Covers P3.8.e · Verifies HC-2, HC-1, Spec §6*

Run the whole suite with distinctive markers as the password and as file content, capturing both streams from every command, including every failure path.
**Verdict:** neither marker appears anywhere in any output.

---

## Coverage

FR-24 is not provoked from the CLI — see the ToDo's coverage note. Interactive prompting (P3.4.c) is checked by hand: run `veil list` on a vault from a real shell, confirm the password does not echo and that the correct one opens the vault.
