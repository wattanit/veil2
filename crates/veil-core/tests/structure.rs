//! Phase 0 test cases T0.2, T0.3, and T0.4 — structural constraints
//! (A-1, FR-2, Spec §1, §6).
//!
//! These check properties of the dependency graph and the source tree rather
//! than of behaviour. Each is a constraint a human reviewer could enforce, and
//! a constraint only a human reviewer enforces is a preference with good
//! intentions.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Every crate reachable from `veil-core`, transitively.
///
/// Resolved from `cargo metadata` rather than from the manifest, because a
/// banned crate arriving as somebody else's transitive dependency is the case
/// that matters — the original Veil's `anyhow` dependency was direct, but
/// nothing guarantees the next one will be.
fn veil_core_dependency_names() -> HashSet<String> {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--all-features"])
        .current_dir(manifest_dir())
        .output()
        .expect("cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).expect("metadata");
    let nodes = metadata["resolve"]["nodes"].as_array().expect("nodes");

    let mut edges: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut name_of: HashMap<&str, String> = HashMap::new();
    for node in nodes {
        let id = node["id"].as_str().expect("id");
        let deps = node["dependencies"]
            .as_array()
            .expect("dependencies")
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect();
        edges.insert(id, deps);
    }
    for package in metadata["packages"].as_array().expect("packages") {
        let id = package["id"].as_str().expect("package id");
        let name = package["name"].as_str().expect("package name").to_owned();
        name_of.insert(id, name);
    }

    let root = *edges
        .keys()
        .find(|id| name_of.get(*id).map(String::as_str) == Some("veil-core"))
        .expect("veil-core is in the resolve graph");

    let mut seen = HashSet::new();
    let mut queue = vec![root];
    while let Some(id) = queue.pop() {
        for dep in edges.get(id).into_iter().flatten() {
            if seen.insert((*dep).to_owned()) {
                queue.push(dep);
            }
        }
    }
    seen.iter()
        .filter_map(|id| name_of.get(id.as_str()).cloned())
        .collect()
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// T0.2 — `veil-core` cannot flatten its errors (FR-2, Spec §6).
///
/// Regression test for the original Veil, whose `From<anyhow::Error>`
/// conversion made a wrong password and a corrupted vault the same value to
/// every caller. `cargo deny` bans the crate as well; this asserts it at the
/// level of the graph the library actually links, so that a change to the deny
/// configuration cannot quietly reopen it.
#[test]
fn t0_2_veil_core_does_not_depend_on_anyhow() {
    let deps = veil_core_dependency_names();
    assert!(
        !deps.contains("anyhow"),
        "veil-core's dependency graph contains anyhow (Spec §6)"
    );
}

/// T0.3 — `veil-core` cannot prompt (A-1).
///
/// The original Veil's untestability came from a dependency, not from a design
/// document that permitted one: its logic sat in the CLI layer coupled to
/// `rpassword`, which is why it has fourteen unit tests, no integration tests,
/// and cannot be exercised without a pseudo-terminal.
#[test]
fn t0_3_veil_core_has_no_interactive_dependency() {
    const INTERACTIVE: &[&str] = &[
        "rpassword",
        "dialoguer",
        "inquire",
        "requestty",
        "promptly",
        "termion",
        "crossterm",
        "console",
        "ratatui",
        "cursive",
    ];

    let deps = veil_core_dependency_names();
    for crate_name in INTERACTIVE {
        assert!(
            !deps.contains(*crate_name),
            "veil-core's dependency graph contains {crate_name}, which reads a \
             terminal (A-1)"
        );
    }
}

/// T0.4 — `crypto` depends on no sibling module (Spec §1).
///
/// The Specification's claim that splitting `crypto` into its own crate for
/// independent audit stays a mechanical move is only true while this holds,
/// and it stops being true the first time it is violated by accident.
#[test]
fn t0_4_crypto_module_has_no_sibling_imports() {
    const SIBLINGS: &[&str] = &["format", "store", "index", "vault", "error"];

    let crypto_dir = manifest_dir().join("src/crypto");
    let mut checked = 0;
    for entry in walk(&crypto_dir) {
        let source = std::fs::read_to_string(&entry).expect("read crypto source");
        checked += 1;
        for sibling in SIBLINGS {
            for pattern in [
                format!("crate::{sibling}"),
                format!("super::{sibling}"),
                format!("use crate::{sibling}"),
            ] {
                assert!(
                    !source.contains(&pattern),
                    "{} refers to sibling module `{sibling}` via `{pattern}` \
                     (Spec §1)",
                    entry.display()
                );
            }
        }
    }
    assert!(checked > 0, "no crypto sources were checked");
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(walk(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
    found
}
