# Veil2 — Phase 3 Test Cases: Command-Line Application

**Version:** 1.0
**Status:** draft
**Date:** 2026-08-08
**Owner:** wattanit
**Foundation and plan versions these cases are built against (G-14):**
- Requirements Document **v1.1** — upstream
- Design Guideline **v1.1** — upstream
- Technical Specification **v1.2** — upstream
- Implementation Plan **v1.4** — upstream
- [Phase3-ToDo.md](Phase3-ToDo.md) **v1.0** — companion; each case names the item it covers

This document owns the **enumerated checks that close Phase 3**. Every case cites the requirement it verifies (G-10).

---

## Conventions

**Case identifiers** are `T<phase>.<n>` — section-numbering references, not foundation identifiers (G-19).

**Every case runs the built binary as a subprocess.** Phase 3's subject is a process: its exit code, its two output streams, and its behaviour with no terminal attached. Calling the library directly would test something other than the product.

**Every case asserts the exit code**, not merely that the command failed. "Non-zero" is satisfied by a binary that fails at everything.

**Nothing here reads a terminal.** A test harness has no tty, which makes the non-interactive path the default the suite runs. Cases that require a terminal to be present are marked and are the two the suite cannot cover; they are checked by hand and said so, rather than faked with a pseudo-terminal.

**Where these run.** The development machine, macOS. Windows and Linux are unconfirmed (Spec §8.1). It matters here for signal delivery (T3.18), for the no-tty detection (T3.12, T3.17), and for file permissions on a password file.

---

## The command surface

### T3.1 — Every core capability is reachable from the shell
*Covers P3.1.a, P3.1.b, P3.1.e · Verifies A-4, FR-1 … FR-21, FR-33*

Run a full lifecycle using nothing but the binary: create a vault, add a file, add a folder, list, save a copy out, replace a file, check for damage, read the statistics, change the password, reopen under the new password, delete a file.
**Verdict:** every step exits 0 and the vault is intact at the end. Any capability that needs the library to reach it is a parity defect (A-4), which is this phase's central requirement rather than a nicety.

### T3.2 — No command accepts a password as an argument
*Covers P3.4.b · Verifies HC-2, Spec §5.2*

Pass `--password secret` to every subcommand.
**Verdict:** the parser rejects it on all of them, as an unknown argument. Arguments appear in process listings and shell history; an option that exists and is merely discouraged is one `history | grep` away from being the disclosure.

### T3.3 — Nothing schedules or conditions an operation
*Covers P3.1.g · Verifies FR-23, Spec §5.2*

Search the full help output of every command for a flag that schedules, times, daemonises, or triggers on a threshold.
**Verdict:** none exists. FR-23 forbids automatic compaction, and a switch a user can wire into cron is that prohibition defeated under another name.

### T3.4 — A path matching two files is refused
*Covers P3.1.c · Verifies FR-13, HC-4*

Add the same file twice under the same folder, then save a copy of that path, then delete it.
**Verdict:** both refuse, naming how many files matched, and the vault is unchanged. Acting on an arbitrary one of two would make delete a coin toss on the user's data.

### T3.5 — A path matching nothing is not reported as damage
*Covers P3.1.d · Verifies FR-2, HC-3*

Ask for a file that was never added.
**Verdict:** a message saying no file has that path, and an exit code distinct from both the damage code and the wrong-password code. A typed path is the commonest mistake there is; sending it to "your vault may be corrupted" is the FR-2 failure one level down.

### T3.31 — A folder add reports what it skipped
*Covers P3.1.e · Verifies FR-10, FR-11*

Add a folder containing regular files, a symbolic link to a file outside it, and a nested folder.
**Verdict:** exit 0, every regular file stored with its relative folder recorded, and the symbolic link named in the output as skipped. A link silently omitted is a file the user believes is in the vault.

---

## Checking for damage

### T3.20 — Damage is found, named, and exits non-zero
*Covers P3.1a.a, P3.1a.b, P3.1a.c, P3.1a.d · Verifies FR-33, S-4, Spec §5.2*

Build a vault of several files, check it, then corrupt the stored bytes of two of them and check again.
**Verdict:** the first run exits 0. The second exits with the damage code, names **both** files, and does not stop at the first — S-4 is about a user learning the full cost in one pass. The output says plainly that Veil2 cannot recover them, because the next thing that person does is decide whether to go looking for a backup.

---

## Output

### T3.6 — The table is in the fixed column order
*Covers P3.2.a, P3.2.b, P3.1.f · Verifies Design §3.4, §7, FR-6, FR-7*

List a vault holding files across several folders, then list with a folder filter and with a name filter.
**Verdict:** columns in the order Design fixes, sizes human-readable, the count exact and unrounded, and each filter returning exactly the matching rows.

### T3.7 — Stored names are printed exactly
*Covers P3.2.c, P3.2.d · Verifies HC-8*

Add files named in Latin, Thai, Arabic, Han and emoji, and list them.
**Verdict:** every name comes back byte-identical to what was stored. Column alignment for double-width scripts is **not** asserted — P3.2.d states the limitation instead of hiding it, and a test that demanded alignment would push the implementation toward padding names, which is the one thing HC-8 forbids.

### T3.8 — Machine output carries the same facts
*Covers P3.3.a, P3.3.d · Verifies Design §3.4*

List and read the statistics in `--format json`, then run a failing command in the same mode.
**Verdict:** the output parses, exact byte counts rather than human-readable sizes, the same set of facts as the table, and the failure is reported in machine form too. A script that meets prose only when something goes wrong has no error handling.

### T3.9 — The streams stay separated
*Covers P3.3.b, P3.3.c · Verifies Design §3.4*

Run a listing with standard output captured to a file and standard error discarded.
**Verdict:** the file parses as valid machine output from first byte to last, with no progress, no banner, and no prose interleaved.

### T3.32 — The reported statistics are the vault's
*Covers P3.2.e · Verifies FR-8, FR-22*

Add files, delete one, and read the statistics.
**Verdict:** entry count, logical bytes, physical bytes, reclaimable bytes and reclaimable share all match what the library reports for the same vault, and the deleted file's bytes appear as reclaimable rather than as gone.

---

## Password input

### T3.10 — A password file works and is the only file read
*Covers P3.4.a, P3.4.c · Verifies Spec §5.2*

Open a vault with `--password-file`.
**Verdict:** exit 0. No prompt is attempted — with no terminal attached, attempting one is how a scripted invocation hangs forever at 3 a.m.

### T3.11 — The environment variable works
*Covers P3.4.a · Verifies Spec §5.2*

Open the same vault with the password in the environment and no file given.
**Verdict:** exit 0, and the file source takes precedence when both are present.

### T3.12 — A non-interactive invocation with no password fails fast
*Covers P3.4.d, P3.7.c · Verifies Design §3.4, Spec §5.2*

Run a command needing a password with no terminal, no file, and no environment variable, under a timeout.
**Verdict:** it exits before the timeout, with the missing-input code, naming which input was missing. Blocking on a prompt nobody can answer is the failure this case exists to catch, so the timeout is the assertion.

### T3.13 — The password never appears in the process
*Covers P3.4.b · Verifies HC-2*

Run a long operation with the password supplied by file, and read the command line of the running process.
**Verdict:** the password is not in it. This is the case that makes T3.2 more than an argument-parser check.

### T3.14 — A wrong password is distinguishable from a damaged vault
*Covers P3.4.g · Verifies FR-2*

Open a good vault with the wrong password. Then damage a vault's header and open it with the right one.
**Verdict:** two different exit codes and two different messages. This is the original Veil's defining failure and the reason FR-2 is worded the way it is; a script that cannot tell them apart sends its user to the wrong remedy, and so does a person.

### T3.15 — A password file is trimmed exactly once
*Covers P3.4.e · Verifies Spec §5.2*

Create a vault with a password file written by `echo` (one trailing newline). Reopen using a file with no trailing newline, then one with two.
**Verdict:** the first two open; the third does not. Trimming all trailing whitespace would silently change a password that legitimately ends in a space, and a password a user cannot reproduce is HC-7 arriving by accident.

### T3.33 — Creation states unrecoverability before it creates
*Covers P3.1.h, P3.4.f · Verifies HC-7, FR-1, FR-29, C-4*

Create a vault. Then attempt one with a password below the C-4 minimum.
**Verdict:** the output states that a lost password cannot be recovered, and states it before the vault exists rather than after. The short password is refused, naming the minimum.

---

## Progress and cancellation

### T3.16 — Progress goes to standard error, results to standard output
*Covers P3.5.a, P3.3.c · Verifies Design §3.4*

Add a file large enough to produce progress, capturing the streams separately.
**Verdict:** progress appears only on standard error; standard output carries only the result. A pipeline that has to strip progress out of its input is a pipeline that will strip the wrong line one day.

### T3.17 — Off a terminal, progress is periodic lines
*Covers P3.5.b, P3.5.c, P3.7.c · Verifies Design §3.4*

Run the same add with standard error captured to a file.
**Verdict:** plain lines, no carriage returns and no escape sequences, and no more than one line per interval. A log full of terminal control characters is the reason this rule exists.

### T3.18 — An interrupt cancels rather than kills
*Covers P3.5.d, P3.5.e · Verifies FR-14, FR-19, HC-4*

Start an add of a large file, send an interrupt mid-operation, and wait for the process.
**Verdict:** the cancelled exit code, a message saying what was left behind, and a vault whose contents and statistics are exactly what they were before the command started (FR-14). Reopening it succeeds. This is the case that proves the CLI can reach the cancellation machinery Phase 2 built rather than merely dying safely.

---

## Exit codes and refusals

### T3.19 — One code per error class
*Covers P3.6.a, P3.6.b, P3.6.c · Verifies Spec §5.2, §6, FR-2*

Provoke each condition in the P3.6 table that this phase can reach — success, usage error, wrong password, not a vault, damage, vault in use, changed on disk, read-only, limit exceeded, cancelled, missing password.
**Verdict:** each produces its own code, no two conditions share one, and the codes appear in the help output. The mapping is exhaustive over the error enum, so a variant added later cannot quietly land in "unexpected failure".

### T3.21 — A vault already open is reported as in use
*Covers P3.6.a · Verifies FR-26*

Hold a vault open, then run a command against the same directory.
**Verdict:** the in-use code, and a message saying the vault is open — not a damage report and not an I/O failure.

### T3.22 — A vault changed on disk refuses the write
*Covers P3.6.a · Verifies FR-27*

Open a vault, change it from a second process, and attempt a write from the first.
**Verdict:** the changed-on-disk code, the write not applied, and the message saying the vault can be reloaded. Detecting a change and refusing is only useful if the way forward is stated.

### T3.23 — A read-only vault reads but does not write
*Covers P3.6.a · Verifies Spec §4.5, §4.8, FR-32*

Make a vault directory read-only. List it, check it, then try to add.
**Verdict:** listing and checking succeed; the add exits with the read-only code. Nothing is wrong with the vault and the message must not suggest a failing disk — the operation that diagnoses a bad drive has to be the one operation a bad drive can still run.

### T3.24 — A destination file is never overwritten unasked
*Covers P3.1.h · Verifies FR-18*

Save a copy to a path that already holds a file, without a pre-confirmation flag, then with one.
**Verdict:** the first refuses, naming the file it would have overwritten; the second proceeds. The original Veil overwrote silently, and a failed save destroyed the user's only good copy.

### T3.25 — A failed save leaves no partial file
*Covers P3.1a.a · Verifies FR-17, HC-3*

Corrupt a file's stored bytes and save a copy of it to a fresh path.
**Verdict:** the damage code, and **nothing at the destination**. A truncated plaintext left on disk is indistinguishable from a short file, and the user finds out when they need it.

### T3.26 — Adding says the original is still there
*Covers P3.1.h · Verifies FR-9, FR-29*

Add a file.
**Verdict:** the source file exists, unmodified, and the output says so. FR-29 puts this at the moment it happens because an unprotected copy the user has forgotten is how data leaves Veil2's protection.

### T3.27 — Deleting says the bytes remain
*Covers P3.1.h · Verifies FR-21, FR-29, FR-22*

Delete a file.
**Verdict:** it is gone from the listing, the reclaimable figure rises, and the output states that the stored bytes remain until space is reclaimed. A user who deletes a file and then hands the vault to someone else must not believe those bytes are gone.

### T3.28 — A limit names both numbers
*Covers P3.6.a · Verifies FR-15*

Add a file larger than the configured per-file limit.
**Verdict:** the limit code, and a message carrying both the limit and the actual size. "Too large" without the two numbers leaves the user to guess what would fit.

---

## Audits

### T3.29 — The vocabulary holds across the whole surface
*Covers P3.1.a, P3.7.d · Verifies Design §7*

Collect every command's help text and every message the suite produces, and search for the forbidden column of Design §7's table: container, archive, entry, object, item, directory, passphrase, import, ingest, export, decrypt, extract, verify, validate, compact, vacuum — and the forbidden claims, "military-grade" through "your data is safe".
**Verdict:** none appears. One word per thing, GUI and CLI alike, is a product decision; the CLI is where it erodes first because the implementation's own vocabulary is right there in the source.

### T3.30 — No output discloses what it must not
*Covers P3.7.e · Verifies HC-2, HC-1, Spec §6*

Run the whole suite with distinctive markers as the password and as file content, capturing both streams from every command, including every failure path.
**Verdict:** neither marker appears anywhere in any output. Error text is where key material escapes, because the failure paths are the ones nobody reads.

---

## Withdrawn

**T3.34 — Interactive prompting.** *Withdrawn, not deferred.* P3.4.c's no-echo prompt needs a terminal, and a harness has none. Driving it through a pseudo-terminal would put the original Veil's central defect back into this project — logic exercisable only through a fake terminal — which is the thing A-1 exists to prevent. The prompt is checked by hand: run `veil list` on a vault from a real shell, confirm the password does not echo and that the correct one opens the vault. Said plainly rather than covered by a test that tests the harness.
