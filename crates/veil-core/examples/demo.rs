//! A walk-through you can run and watch: `cargo run --example demo`
//!
//! Nothing here is a test. It uses only the public API, prints what it does,
//! and deliberately damages a vault so you can see what the failure looks
//! like. Everything lands in a temporary directory it prints and removes.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::path::Path;

use veil_core::crypto::{KdfParams, Password};
use veil_core::vault::{Progress, ProgressReport, Unit, Vault};
use veil_core::{Cancel, Error, NoProgress};

/// Prints a progress bar, because a progress sink you cannot see proves nothing.
struct Bar {
    label: &'static str,
    last: u64,
}

impl Progress for Bar {
    fn report(&mut self, r: ProgressReport) {
        let unit = match r.unit {
            Unit::Bytes => "bytes",
            Unit::Entries => "entries",
        };
        match r.total {
            Some(total) if total > 0 => {
                let pct = r.done * 100 / total;
                print!(
                    "\r    {} {pct:>3}%  {} of {total} {unit}   ",
                    self.label, r.done
                );
            }
            _ => print!("\r    {} {} {unit}   ", self.label, r.done),
        }
        use std::io::Write as _;
        let _ = std::io::stdout().flush();
        self.last = r.done;
    }
}

fn heading(n: u8, text: &str) {
    println!("\n\x1b[1m{n}. {text}\x1b[0m");
}

fn main() {
    let scratch = std::env::temp_dir().join(format!("veil2-demo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();
    let vault_dir = scratch.join("Demo.veil");

    println!("Working in {}", scratch.display());
    println!("(removed when this finishes)");

    // Source files, so the ingest is of real files on disk.
    let sources = scratch.join("sources");
    std::fs::create_dir_all(sources.join("photos/2024")).unwrap();
    std::fs::write(sources.join("notes.txt"), b"a short note").unwrap();
    std::fs::write(sources.join("photos/holiday.jpg"), big(3_500_000)).unwrap();
    std::fs::write(sources.join("photos/2024/beach.jpg"), big(900_000)).unwrap();
    let link = sources.join("photos/shortcut");
    #[cfg(unix)]
    let made_link = std::os::unix::fs::symlink(sources.join("notes.txt"), &link).is_ok();
    #[cfg(not(unix))]
    let made_link = false;

    let password = Password::new("correct horse battery staple".to_owned());

    // --- 1 -----------------------------------------------------------------
    heading(1, "Create a vault");
    // A small pack cap so you can watch it span several pack files.
    let mut vault =
        Vault::create(&vault_dir, &password, KdfParams::for_tests(), 1_000_000).unwrap();
    println!(
        "    created, format version {}",
        vault.header().format_version
    );
    println!("    files on disk: {}", list(&vault_dir));
    println!("    (test key-derivation cost, so this returns instantly)");

    // --- 2 -----------------------------------------------------------------
    heading(2, "Add a folder");
    let outcome = vault
        .add_folder(
            &sources,
            &mut Bar {
                label: "stored",
                last: 0,
            },
            &Cancel::new(),
        )
        .unwrap();
    println!();
    println!("    {} entries stored", outcome.added.len());
    for entry in vault.entries() {
        let path = if entry.folder.is_empty() {
            entry.name.clone()
        } else {
            format!("{}/{}", entry.folder, entry.name)
        };
        println!("      {:<32} {:>9} bytes", path, entry.size);
    }
    if made_link {
        println!("    skipped, and told you about it:");
        for s in &outcome.skipped {
            println!("      {} — {:?}", s.path.display(), s.reason);
        }
    }
    println!("    pack files: {:?}", packs(&vault_dir));

    // --- 3 -----------------------------------------------------------------
    heading(3, "Statistics, without reading any stored data");
    let s = vault.statistics();
    println!("    entries          {}", s.entry_count);
    println!("    logical bytes    {}", s.logical_bytes);
    println!("    physical bytes   {}", s.physical_bytes);
    println!("    reclaimable      {}", s.reclaimable_bytes);

    // --- 4 -----------------------------------------------------------------
    heading(4, "Extract one file and compare it to the original");
    let target = vault.find("photos", "holiday.jpg").unwrap().id;
    let out = scratch.join("recovered.jpg");
    vault
        .extract_to_path(
            target,
            &out,
            &mut Bar {
                label: "read",
                last: 0,
            },
            &Cancel::new(),
        )
        .unwrap();
    println!();
    let original = std::fs::read(sources.join("photos/holiday.jpg")).unwrap();
    let recovered = std::fs::read(&out).unwrap();
    println!(
        "    {} bytes out, identical to the original: {}",
        recovered.len(),
        original == recovered
    );

    // --- 5 -----------------------------------------------------------------
    heading(5, "The original source files are untouched");
    println!(
        "    notes.txt still there: {}",
        sources.join("notes.txt").exists()
    );
    println!(
        "    holiday.jpg unchanged: {}",
        std::fs::read(sources.join("photos/holiday.jpg")).unwrap() == original
    );

    // --- 6 -----------------------------------------------------------------
    heading(
        6,
        "Delete a file, and see the vault say the bytes are still there",
    );
    let doomed = vault.find("", "notes.txt").unwrap().id;
    let before = vault.statistics();
    vault.delete(doomed).unwrap();
    let after = vault.statistics();
    println!(
        "    entries {} -> {},  physical bytes {} -> {} (unchanged),  reclaimable {} -> {}",
        before.entry_count,
        after.entry_count,
        before.physical_bytes,
        after.physical_bytes,
        before.reclaimable_bytes,
        after.reclaimable_bytes
    );
    println!("    the bytes stay until compaction, and the figures say so");

    // --- 7 -----------------------------------------------------------------
    heading(
        7,
        "A second program cannot open the vault while this one holds it",
    );
    match Vault::open(&vault_dir, &password) {
        Err(Error::VaultInUse) => println!("    refused: {}", Error::VaultInUse),
        Err(e) => println!("    UNEXPECTED: {e}"),
        Ok(_) => println!("    UNEXPECTED: it opened twice"),
    }

    // --- 8 -----------------------------------------------------------------
    heading(8, "Change the password");
    let new_password = Password::new("a different long passphrase".to_owned());
    vault
        .change_password(&password, &new_password, KdfParams::for_tests())
        .unwrap();
    println!("    done. closing the vault.");
    vault.lock();

    match Vault::open(&vault_dir, &password) {
        Err(Error::WrongPassword) => println!("    old password: {}", Error::WrongPassword),
        Err(e) => println!("    UNEXPECTED: {e}"),
        Ok(_) => println!("    UNEXPECTED: the old password still opens it"),
    }
    let vault = Vault::open(&vault_dir, &new_password).unwrap();
    println!("    new password: opens, {} entries", vault.entries().len());

    // --- 9 -----------------------------------------------------------------
    heading(9, "Verify the whole vault");
    let report = vault
        .verify(
            &mut Bar {
                label: "verified",
                last: 0,
            },
            &Cancel::new(),
        )
        .unwrap();
    println!();
    println!(
        "    complete: {}, all passed: {}, entries checked: {}",
        report.complete,
        report.all_passed(),
        report.verdicts.len()
    );
    drop(vault);

    // --- 10 ----------------------------------------------------------------
    heading(
        10,
        "Now break it on purpose: flip one byte in one pack file",
    );
    let victim = {
        let v = Vault::open(&vault_dir, &new_password).unwrap();
        let e = v
            .entries()
            .iter()
            .find(|e| e.name == "holiday.jpg")
            .unwrap();
        (e.id, e.name.clone(), e.extents[0])
    };
    let pack = veil_core::store::pack_path(&vault_dir, victim.2.pack_id);
    let mut bytes = std::fs::read(&pack).unwrap();
    let at = usize::try_from(victim.2.offset).unwrap() + 100;
    println!(
        "    pack {:06}, byte {at}: {:#04x} -> {:#04x}",
        victim.2.pack_id,
        bytes[at],
        bytes[at] ^ 0x01
    );
    bytes[at] ^= 0x01;
    std::fs::write(&pack, bytes).unwrap();

    heading(11, "What the damage costs");
    let vault = Vault::open(&vault_dir, &new_password).expect("the vault still opens");
    println!(
        "    vault opens: yes, {} entries listed",
        vault.entries().len()
    );

    let out = scratch.join("should-not-exist.jpg");
    match vault.extract_to_path(victim.0, &out, &mut NoProgress, &Cancel::new()) {
        Err(Error::Corrupt { what, affected }) => {
            println!(
                "    extracting the damaged file: {}",
                Error::Corrupt {
                    what,
                    affected: affected.clone()
                }
            );
            println!("      damaged component: {what}");
            println!(
                "      entries affected:  {affected:?}   (that is {})",
                victim.1
            );
        }
        other => println!("    UNEXPECTED: {other:?}"),
    }
    println!(
        "    partial output left behind: {}   <- must be false",
        out.exists()
    );

    let report = vault.verify(&mut NoProgress, &Cancel::new()).unwrap();
    println!("    whole-vault verify:");
    for v in &report.verdicts {
        let name = &vault.entries().iter().find(|e| e.id == v.id).unwrap().name;
        let mark = if v.outcome == veil_core::vault::Outcome::Passed {
            "ok    "
        } else {
            "FAILED"
        };
        println!("      {mark} {name}");
    }
    println!("    it did not stop at the first casualty, and it named the one that failed");

    heading(12, "Everything else still comes out intact");
    for entry in vault.entries() {
        if entry.id == victim.0 {
            continue;
        }
        let mut buf = Vec::new();
        let ok = vault
            .extract(entry.id, &mut buf, &mut NoProgress, &Cancel::new())
            .is_ok();
        println!(
            "      {:<32} {} ({} bytes)",
            entry.name,
            if ok { "ok" } else { "FAILED" },
            buf.len()
        );
    }

    heading(
        13,
        "And a wrong password on a damaged vault is still a wrong password",
    );
    drop(vault);
    match Vault::open(&vault_dir, &Password::new("nope nope nope nope".to_owned())) {
        Err(Error::WrongPassword) => println!("    {}", Error::WrongPassword),
        Err(e) => println!("    UNEXPECTED: {e}"),
        Ok(_) => println!("    UNEXPECTED: a wrong password opened it"),
    }
    println!("    — not \"corrupted vault\", which is where the original Veil sent you");

    let _ = std::fs::remove_dir_all(&scratch);
    println!("\nDone. Temporary directory removed.");
}

fn big(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 7 % 251) as u8).collect()
}

fn list(dir: &Path) -> String {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names.join(", ")
}

fn packs(dir: &Path) -> Vec<String> {
    veil_core::store::existing_pack_ids(dir)
        .unwrap()
        .into_iter()
        .map(|id| {
            let len = std::fs::metadata(veil_core::store::pack_path(dir, id))
                .unwrap()
                .len();
            format!("{id:06}.pack ({len} bytes)")
        })
        .collect()
}
