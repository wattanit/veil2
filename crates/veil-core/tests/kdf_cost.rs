//! Phase 1 to-do P1.12 — what the key derivation actually costs (C-3).
//!
//! Ignored by default and run on request: it is a measurement, not a check,
//! and its result is a number for the owner rather than a pass or a fail.
//!
//! ```text
//! cargo test -p veil-core --test kdf_cost -- --ignored --nocapture
//! ```
//!
//! C-3 asks for roughly one second on contemporary desktop hardware. The
//! measurement below is one machine's; the parameters a vault is created with
//! are stored in its header (HC-5), so a later change never strands a vault
//! written under the old ones.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::Instant;

use veil_core::crypto::{KdfAlgorithm, KdfParams, Password, derive_kek};

#[test]
#[ignore = "a measurement; run it on request"]
fn what_the_derivation_costs() {
    let password = Password::new("a sufficiently long password".to_owned());
    let salt = [7u8; 32];

    println!("\n  m_cost      t_cost  p_cost   time");
    for (m_cost, t_cost, p_cost) in [
        (64 * 1024, 1, 1),
        (64 * 1024, 2, 1),
        (64 * 1024, 3, 1),
        (64 * 1024, 3, 4),
        (128 * 1024, 2, 1),
        (128 * 1024, 3, 4),
        (256 * 1024, 1, 4),
        (256 * 1024, 3, 4),
    ] {
        let params = KdfParams {
            m_cost,
            t_cost,
            p_cost,
        };
        let started = Instant::now();
        let _ = derive_kek(KdfAlgorithm::Argon2id, params, &salt, &password).unwrap();
        let elapsed = started.elapsed();
        println!(
            "  {:>4} MiB  {t_cost:>6}  {p_cost:>6}   {:>6.2}s{}",
            m_cost / 1024,
            elapsed.as_secs_f64(),
            if params == KdfParams::for_new_vaults() {
                "   <- what new vaults use"
            } else {
                ""
            }
        );
    }
    println!();
}
