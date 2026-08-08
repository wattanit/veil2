//! A crash-test subject for reclaiming space. **Not the product.**
//!
//! Reclaiming space is multi-pack behaviour and the pack cap is 1 GiB. Spec
//! §4.5 made the cap a value the API accepts precisely so a test needing
//! several packs would not need several gigabytes — and the command line does
//! not expose it, because a flag whose only purpose is to make a test cheap is
//! a seam in shipped code, which this project does not add.
//!
//! So the crash tests for that one operation kill this instead. It is a real
//! process doing real work through the real library, and it is killed for
//! real; nothing here is simulated. It is an example rather than a binary
//! target, so it is never installed and never shipped.
//!
//! ```text
//! reclaim_subject <vault> <password-file> <cap> setup <count> <size>
//! reclaim_subject <vault> <password-file> <cap> reclaim
//! ```
//!
//! The password comes from a file, never from an argument — HC-2 does not stop
//! applying because the caller is a test.

// A test fixture, not shipped code. The lints that keep panics out of the
// product are pointed at the product; here a bad argument should stop loudly.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use veil_core::crypto::{KdfParams, Password};
use veil_core::vault::{Cancel, NoProgress, Vault};

/// Cheap in debug, real in release. `for_tests` is compiled out of release
/// builds so that no release binary can make a weak vault, and this one is
/// built both ways.
fn params() -> KdfParams {
    #[cfg(debug_assertions)]
    {
        KdfParams::for_tests()
    }
    #[cfg(not(debug_assertions))]
    {
        KdfParams::for_new_vaults()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 5 {
        eprintln!("usage: reclaim_subject <vault> <password-file> <cap> <command> [args]");
        std::process::exit(2);
    }

    let dir = PathBuf::from(&args[1]);
    let password = read_password(Path::new(&args[2]));
    let cap: u64 = args[3].parse().expect("cap must be a number");

    match args[4].as_str() {
        "setup" => {
            let count: usize = args[5].parse().unwrap();
            let size: usize = args[6].parse().unwrap();
            setup(&dir, &password, cap, count, size);
        }
        "reclaim" => {
            let mut vault = Vault::open(&dir, &password).expect("open");
            let reclaimed = vault
                .compact(&mut NoProgress, &Cancel::new())
                .expect("reclaim");
            println!("{}", reclaimed.bytes_recovered);
        }
        other => {
            eprintln!("unknown command {other}");
            std::process::exit(2);
        }
    }
}

/// The same one-trailing-newline rule the command line uses.
fn read_password(path: &Path) -> Password {
    let text = std::fs::read_to_string(path).expect("password file");
    Password::new(text.strip_suffix('\n').unwrap_or(&text).to_owned())
}

/// A vault with several packs, half of it garbage, so reclaiming has real work
/// to do and a kill lands somewhere in the middle of it.
fn setup(dir: &Path, password: &Password, cap: u64, count: usize, size: usize) {
    let mut vault = Vault::create(dir, password, params(), cap).expect("create");
    let mut ids = Vec::new();
    for n in 0..count {
        let content: Vec<u8> = (0..size).map(|i| ((i + n) % 251) as u8).collect();
        ids.push(
            vault
                .add(
                    &format!("f{n}.bin"),
                    "d",
                    &mut content.as_slice(),
                    &mut NoProgress,
                    &Cancel::new(),
                )
                .expect("add"),
        );
    }
    for id in ids.iter().step_by(2) {
        vault.delete(*id).expect("delete");
    }
    println!("{}", vault.statistics().reclaimable_bytes);
}
