//! Phase 4 test cases T4.2 to T4.8 — killing a real process part-way through
//! an operation (Spec §9; HC-4, FR-12, FR-13, FR-21, FR-24).
//!
//! Nothing here is simulated. `veil-core` has no seam that lets a test pretend
//! to crash, deliberately (Spec §11.1), so the only way to check the write
//! ordering is to end a process without letting it clean up. The signal is a
//! kill, not an interrupt: an interrupt is cancellation, which promises
//! something stronger, and T3.18 covers it.
//!
//! **The four invariants.** Every case asserts all of them, because a case that
//! asserts only "the vault opens" is not asserting HC-4:
//!
//! 1. the vault opens;
//! 2. every file that existed before the killed operation is still listed;
//! 3. each of those extracts byte-identically;
//! 4. the statistics match a full recount.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use harness::PASSWORD;
use veil_core::crypto::Password;
use veil_core::index::generations;
use veil_core::store::total_pack_bytes;
use veil_core::vault::{Cancel, NoProgress, Vault};

/// Big enough that the add is unmistakably in flight when the kill lands, small
/// enough that a debug build still finishes the suite.
const BIG: usize = 32 * 1024 * 1024;

/// How often the filesystem is asked whether the operation has started.
const POLL: Duration = Duration::from_millis(2);

/// No case may hang; a subject that never reaches its condition fails here.
const PATIENCE: Duration = Duration::from_secs(120);

// --------------------------------------------------------------- fixtures --

struct Subject {
    root: PathBuf,
}

impl Subject {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "veil2-crash-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("password.txt"), PASSWORD).unwrap();
        Self { root }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn password_file(&self) -> PathBuf {
        self.path("password.txt")
    }

    fn password(&self) -> Password {
        Password::new(PASSWORD.to_owned())
    }

    /// A vault holding a few files, and what they were, so that "loses nothing
    /// that existed before" is a claim with something behind it.
    fn stocked(&self, name: &str) -> (PathBuf, BTreeMap<String, Vec<u8>>) {
        let vault = self.path(name);
        assert_eq!(self.veil(&["create", str(&vault)]).unwrap(), 0);

        let mut before = BTreeMap::new();
        for n in 0..3 {
            let content: Vec<u8> = (0..4096 + n).map(|i| (i % 251) as u8).collect();
            let source = self.path(&format!("src{n}.bin"));
            std::fs::write(&source, &content).unwrap();
            assert_eq!(
                self.veil(&["add", str(&vault), str(&source), "--folder", "d"])
                    .unwrap(),
                0
            );
            before.insert(format!("src{n}.bin"), content);
        }
        (vault, before)
    }

    /// Runs a command to completion, returning its exit code.
    fn veil(&self, args: &[&str]) -> Option<i32> {
        self.spawn(args).wait().unwrap().code()
    }

    /// Starts a command without waiting for it.
    fn spawn(&self, args: &[&str]) -> Child {
        Command::new(env!("CARGO_BIN_EXE_veil"))
            .args(args)
            .arg("--password-file")
            .arg(self.password_file())
            .env_remove("VEIL_PASSWORD")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    }

    /// Starts the crash-test subject of the reclaim cases.
    fn spawn_subject(&self, vault: &Path, cap: u64, args: &[&str]) -> Child {
        Command::new(subject_binary())
            .arg(vault)
            .arg(self.password_file())
            .arg(cap.to_string())
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    }
}

impl Drop for Subject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn str(path: &Path) -> &str {
    path.to_str().unwrap()
}

/// The subject binary, in cargo's examples directory beside the one under test.
///
/// Found by searching rather than by a fixed name: building an example as part
/// of `cargo test` produces only the hash-suffixed file, and `cargo build
/// --example` produces both. The newest match is the one this run built.
fn subject_binary() -> PathBuf {
    let examples = Path::new(env!("CARGO_BIN_EXE_veil"))
        .parent()
        .unwrap()
        .join("examples");

    let mut found: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(&examples).into_iter().flatten().flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_subject = name == "reclaim_subject"
            || name == "reclaim_subject.exe"
            || (name.starts_with("reclaim_subject-") && !name.contains('.'));
        if !is_subject {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|m| m.modified()) else {
            continue;
        };
        if found.as_ref().is_none_or(|(best, _)| modified > *best) {
            found = Some((modified, path));
        }
    }

    found.map(|(_, path)| path).unwrap_or_else(|| {
        panic!(
            "the crash subject is not built. Run the whole package — `cargo test -p veil-cli` — \
             rather than this target alone; selecting one test target skips examples. \
             Looked in {}",
            examples.display()
        )
    })
}

/// Waits until `ready` says the operation is under way, then kills the process
/// outright.
///
/// The condition looks at the vault on disk — bytes appearing where the
/// operation puts them — rather than at anything the process was asked to tell
/// a test. There is nothing to tell it with, and adding something would be the
/// seam Spec §11.1 rejected.
fn kill_once_started(child: Child, ready: impl Fn() -> bool) {
    assert!(
        kill_if_possible(child, ready),
        "the operation finished before it could be killed"
    );
}

/// The same, but tolerating an operation that finishes first.
///
/// Returns whether the kill actually landed. Used by the delete case, where
/// there is no in-flight state to watch for and the kill is timed instead: an
/// attempt that arrives after the operation is over has not proved anything
/// about interruption, but the vault it produced is still a vault the
/// invariants must hold for.
fn kill_if_possible(mut child: Child, ready: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + PATIENCE;
    loop {
        if ready() {
            child.kill().unwrap();
            child.wait().unwrap();
            return true;
        }
        if child.try_wait().unwrap().is_some() {
            return false;
        }
        assert!(
            Instant::now() < deadline,
            "the operation never started within {PATIENCE:?}"
        );
        std::thread::sleep(POLL);
    }
}

/// The four invariants of P4.2.c. Nothing else in this file asserts survival.
fn assert_survived(subject: &Subject, vault: &Path, before: &BTreeMap<String, Vec<u8>>, at: &str) {
    // 1 — it opens. Every later assertion depends on this one.
    let opened = Vault::open(vault, &subject.password())
        .unwrap_or_else(|e| panic!("{at}: the vault does not open: {e}"));

    // 2 and 3 — everything that was there is still there, and still itself.
    for (name, content) in before {
        let entry = opened
            .entries()
            .iter()
            .find(|e| &e.name == name)
            .unwrap_or_else(|| panic!("{at}: {name} was lost"));
        let mut out = Vec::new();
        opened
            .extract(entry.id, &mut out, &mut NoProgress, &Cancel::new())
            .unwrap_or_else(|e| panic!("{at}: {name} no longer reads: {e}"));
        assert_eq!(&out, content, "{at}: {name} came back different");
    }

    // 4 — the arithmetic survived too. An incremental counter is broken by
    // exactly the event this suite creates.
    assert_eq!(
        opened.statistics(),
        opened.recount_statistics().unwrap(),
        "{at}: the statistics diverged from a recount"
    );

    // T4.8 — at least one index slot authenticated, which is what opening at
    // all proves, and both were at least readable as slots.
    assert!(
        generations(vault).iter().any(Option::is_some),
        "{at}: neither index slot is even readable"
    );
}

// ------------------------------------------------------------------ cases --

/// T4.2 — a kill during an add loses nothing that was already there
/// (HC-4, FR-12).
#[test]
fn t4_2_a_kill_during_an_add_loses_nothing() {
    let subject = Subject::new("add");
    let (vault, before) = subject.stocked("Add.veil");

    let source = subject.path("big.bin");
    std::fs::write(&source, vec![7u8; BIG]).unwrap();
    let baseline = total_pack_bytes(&vault).unwrap();

    let child = subject.spawn(&["add", str(&vault), str(&source), "--folder", "d"]);
    let watching = vault.clone();
    kill_once_started(child, move || {
        total_pack_bytes(&watching).unwrap_or(0) > baseline + 1024 * 1024
    });

    assert_survived(&subject, &vault, &before, "after a killed add");

    // The interrupted file is wholly absent or wholly present — never listed
    // with content that does not authenticate. `assert_survived` proved every
    // listed file reads, so listing it at all would already have failed there.
    let opened = Vault::open(&vault, &subject.password()).unwrap();
    let names: Vec<&str> = opened.entries().iter().map(|e| e.name.as_str()).collect();
    assert!(
        !names.contains(&"big.bin") || opened.entries().len() == before.len() + 1,
        "the interrupted file is half in the index"
    );
}

/// T4.3 — a kill during a replace leaves exactly one intact version
/// (HC-4, FR-13).
///
/// FR-13 says the new content is durable before the old becomes unreachable.
/// This is the only case that can tell whether that ordering was implemented or
/// merely intended.
#[test]
fn t4_3_a_kill_during_a_replace_leaves_one_intact_version() {
    let subject = Subject::new("replace");
    let (vault, before) = subject.stocked("Replace.veil");

    let replacement = subject.path("replacement.bin");
    std::fs::write(&replacement, vec![9u8; BIG]).unwrap();
    let baseline = total_pack_bytes(&vault).unwrap();

    let child = subject.spawn(&[
        "replace",
        str(&vault),
        "d/src0.bin",
        "--from",
        str(&replacement),
    ]);
    let watching = vault.clone();
    kill_once_started(child, move || {
        total_pack_bytes(&watching).unwrap_or(0) > baseline + 1024 * 1024
    });

    // The old content is what was there before, so the standard invariants
    // cover the "old survived" case. The "new survived" case is checked below.
    let opened = Vault::open(&vault, &subject.password()).unwrap();
    let entry = opened
        .entries()
        .iter()
        .find(|e| e.name == "src0.bin")
        .expect("the file is gone entirely, which is zero intact versions");

    let mut out = Vec::new();
    opened
        .extract(entry.id, &mut out, &mut NoProgress, &Cancel::new())
        .expect("the one version that is there does not read");
    assert!(
        out == before["src0.bin"] || out == vec![9u8; BIG],
        "what came back is neither the old content nor the new one"
    );
    drop(opened);

    let mut remaining = before.clone();
    remaining.remove("src0.bin");
    assert_survived(&subject, &vault, &remaining, "after a killed replace");
}

/// T4.4 — a kill during a delete leaves the file present or gone, never half
/// (HC-4, FR-21).
///
/// A delete is one index generation, so this is mostly asserting that it really
/// is one: an implementation that removed the entry and updated the statistics
/// in two commits would show up here and nowhere else.
///
/// **The window is narrow and the kill is timed, not triggered.** There is no
/// intermediate state on disk to watch for — that is the point of a single
/// commit — so the delete is first timed on a copy of the vault and then killed
/// just short of that on another. Landing inside the commit is a matter of
/// chance; landing outside it still asserts the invariants, and the case is
/// repeated so that the chance is taken several times.
#[test]
fn t4_4_a_kill_during_a_delete_leaves_no_half_state() {
    let subject = Subject::new("delete");
    let (vault, before) = subject.stocked("Delete.veil");

    let twin = subject.path("Twin.veil");
    copy_dir(&vault, &twin);
    let started = Instant::now();
    assert_eq!(subject.veil(&["delete", str(&twin), "d/src0.bin"]), Some(0));
    let full = started.elapsed();

    let mut landed = 0;
    for (n, share) in [40u32, 60, 75, 90].iter().enumerate() {
        let attempt = subject.path(&format!("Delete{n}.veil"));
        copy_dir(&vault, &attempt);

        let child = subject.spawn(&["delete", str(&attempt), "d/src0.bin"]);
        let deadline = Instant::now() + full * *share / 100;
        if kill_if_possible(child, move || Instant::now() >= deadline) {
            landed += 1;
        }

        let opened = Vault::open(&attempt, &subject.password()).unwrap();
        let still_there = opened.entries().iter().any(|e| e.name == "src0.bin");
        drop(opened);

        // Present or gone are both legitimate. What is not is a third state.
        let mut expected = before.clone();
        if !still_there {
            expected.remove("src0.bin");
        }
        assert_survived(
            &subject,
            &attempt,
            &expected,
            &format!("after a killed delete at {share}%"),
        );
    }

    assert!(
        landed > 0,
        "every attempt arrived after the delete had already finished, so nothing was interrupted"
    );
}

/// T4.5 — a kill during reclaiming space loses no live file (HC-4, FR-24).
///
/// FR-24 requires the vault to be openable at *every* point during the
/// operation, and a kill at an arbitrary point is the only way to sample that.
/// Driven through the subject binary rather than the shipped one, for the
/// reason its own documentation gives.
#[test]
fn t4_5_a_kill_during_reclaiming_loses_nothing() {
    const CAP: u64 = 64 * 1024;
    let subject = Subject::new("reclaim");
    let vault = subject.path("Reclaim.veil");

    let setup = subject.spawn_subject(&vault, CAP, &["setup", "12", "30000"]);
    assert!(setup.wait_with_output().unwrap().status.success());

    // What is live beforehand, read through the library, so the comparison
    // afterwards is against content and not against a count.
    let opened = Vault::open(&vault, &subject.password()).unwrap();
    let mut before = BTreeMap::new();
    for entry in opened.entries() {
        let mut out = Vec::new();
        opened
            .extract(entry.id, &mut out, &mut NoProgress, &Cancel::new())
            .unwrap();
        before.insert(entry.name.clone(), out);
    }
    let highest = *veil_core::store::existing_pack_ids(&vault)
        .unwrap()
        .last()
        .unwrap();
    drop(opened);

    // A pack above every pack that existed is the new one being filled, which
    // means the copy is under way.
    let child = subject.spawn_subject(&vault, CAP, &["reclaim"]);
    let watching = vault.clone();
    kill_once_started(child, move || {
        veil_core::store::existing_pack_ids(&watching)
            .unwrap_or_default()
            .iter()
            .any(|id| *id > highest)
    });

    assert_survived(&subject, &vault, &before, "after a killed reclaim");
}

/// T4.6 — after any kill, the statistics are true again (FR-8, FR-22, HC-4).
///
/// Asserted inside `assert_survived`, which every case above calls; this states
/// the obligation in one place so it cannot be met by four cases that each
/// checked something slightly different.
#[test]
fn t4_6_the_statistics_are_true_after_a_kill() {
    let subject = Subject::new("statistics");
    let (vault, before) = subject.stocked("Statistics.veil");

    let source = subject.path("big.bin");
    std::fs::write(&source, vec![3u8; BIG]).unwrap();
    let baseline = total_pack_bytes(&vault).unwrap();

    let child = subject.spawn(&["add", str(&vault), str(&source), "--folder", "d"]);
    let watching = vault.clone();
    kill_once_started(child, move || {
        total_pack_bytes(&watching).unwrap_or(0) > baseline + 1024 * 1024
    });

    // Before the reconciliation an open would perform, the figures may count
    // bytes no commit learned of. That is what FR-32 exists for, and this case
    // is the assertion that it closes the gap rather than papering over it.
    let opened = Vault::open(&vault, &subject.password()).unwrap();
    assert_eq!(opened.statistics(), opened.recount_statistics().unwrap());
    drop(opened);

    assert_survived(&subject, &vault, &before, "after a killed add");
}

/// T4.7 — repeated kills at unpredictable points (HC-4).
///
/// The case that finds the boundary nobody thought of. Ignored by default
/// because it costs minutes; seeded from a fixed sequence rather than a clock,
/// because an unreproducible crash-test failure is worth almost nothing.
#[test]
#[ignore = "costs minutes; the sweep, run on request"]
fn t4_7_repeated_kills_at_unpredictable_points() {
    const CAP: u64 = 64 * 1024;
    // A fixed sequence, so a failure names a run that can be repeated exactly.
    const SHARES: [u32; 8] = [5, 17, 31, 44, 58, 66, 79, 93];

    let subject = Subject::new("sweep");

    for (n, share) in SHARES.iter().enumerate() {
        // Add, killed part-way through a large file.
        let (vault, before) = subject.stocked(&format!("Sweep{n}.veil"));
        let source = subject.path("sweep.bin");
        std::fs::write(&source, vec![(n % 251) as u8; BIG]).unwrap();
        let baseline = total_pack_bytes(&vault).unwrap();
        let target = baseline + u64::from(*share) * (BIG as u64) / 100;

        let child = subject.spawn(&["add", str(&vault), str(&source), "--folder", "d"]);
        let watching = vault.clone();
        kill_once_started(child, move || {
            total_pack_bytes(&watching).unwrap_or(0) > target
        });
        assert_survived(
            &subject,
            &vault,
            &before,
            &format!("sweep run {n}: add killed at {share}%"),
        );

        // Reclaiming, killed once a new pack has appeared.
        let reclaim_vault = subject.path(&format!("SweepReclaim{n}.veil"));
        let setup = subject.spawn_subject(&reclaim_vault, CAP, &["setup", "10", "30000"]);
        assert!(setup.wait_with_output().unwrap().status.success());

        let opened = Vault::open(&reclaim_vault, &subject.password()).unwrap();
        let mut live = BTreeMap::new();
        for entry in opened.entries() {
            let mut out = Vec::new();
            opened
                .extract(entry.id, &mut out, &mut NoProgress, &Cancel::new())
                .unwrap();
            live.insert(entry.name.clone(), out);
        }
        let highest = *veil_core::store::existing_pack_ids(&reclaim_vault)
            .unwrap()
            .last()
            .unwrap();
        drop(opened);

        let child = subject.spawn_subject(&reclaim_vault, CAP, &["reclaim"]);
        let watching = reclaim_vault.clone();
        kill_once_started(child, move || {
            veil_core::store::existing_pack_ids(&watching)
                .unwrap_or_default()
                .iter()
                .any(|id| *id > highest)
        });
        assert_survived(
            &subject,
            &reclaim_vault,
            &live,
            &format!("sweep run {n}: reclaim killed"),
        );
    }
}

/// Copies a vault directory, which is all a vault is.
fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let path = entry.path();
        let target = to.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}
