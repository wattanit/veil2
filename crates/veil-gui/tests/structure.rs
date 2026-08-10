//! Phase 5 test cases T5.5, T5.6, and T5.7, and Phase 6's T6.32 —
//! properties of the shell's configuration and dependency graph rather
//! than of behaviour (Spec §5.3, HC-1, Requirements §2.1, §8).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::process::Command;

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// T5.5 — the Content-Security-Policy admits no remote origin (P5.3.a,
/// Spec §5.3, §7).
#[test]
fn t5_5_the_csp_admits_no_remote_origin() {
    let conf = std::fs::read_to_string(manifest_dir().join("tauri.conf.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&conf).unwrap();
    let csp = json["app"]["security"]["csp"]
        .as_str()
        .expect("tauri.conf.json must set a CSP, not null — Spec §5.3");

    for directive in csp.split(';') {
        let directive = directive.trim();
        if directive.is_empty() {
            continue;
        }
        for source in directive.split_whitespace().skip(1) {
            let allowed = source == "'self'"
                || source == "'unsafe-inline'"
                || source == "data:"
                || source == "ipc:"
                || source == "http://ipc.localhost";
            assert!(
                allowed,
                "CSP directive {directive:?} names a source ({source}) that is \
                 neither the bundle nor Tauri's own IPC channel"
            );
        }
    }
}

/// T5.6 — the frontend source references no persistent storage API (P5.3.b,
/// HC-1, Spec §5.3).
///
/// The same denylist-over-source-tree approach `veil-core`'s T0.1 and T0.2
/// use for its dependency graph, applied to the frontend's own source
/// instead of `cargo metadata`.
#[test]
fn t5_6_the_frontend_source_references_no_persistent_storage_api() {
    const DENYLIST: [&str; 3] = ["localStorage", "sessionStorage", "indexedDB"];

    let src = manifest_dir().join("ui").join("src");
    let mut offending = Vec::new();
    let mut stack = vec![src];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let contents = std::fs::read_to_string(&path).unwrap();
            for banned in DENYLIST {
                if contents.contains(banned) {
                    offending.push(format!("{}: {banned}", path.display()));
                }
            }
        }
    }

    assert!(
        offending.is_empty(),
        "the frontend source references a persistent storage API (HC-1):\n  {}",
        offending.join("\n  ")
    );
}

/// T5.7 — DevTools reach the dependency graph only on an explicit
/// `--features devtools` build, never a plain one (P5.3.c, Spec §5.3).
///
/// A plain `cargo tauri build` — what a release actually runs — never passes
/// that flag, so this is the mechanism that keeps DevTools out of a release.
/// `cfg(debug_assertions)` cannot do this from `Cargo.toml`: a `[target]`
/// dependency table only understands platform predicates, and Cargo silently
/// ignores a profile one placed there instead of rejecting it — confirmed by
/// trying exactly that and reading the warning, not assumed.
#[test]
fn t5_7_devtools_reaches_the_graph_only_with_the_feature_flag() {
    assert!(
        !resolved_tauri_features(&[]).contains("devtools"),
        "the devtools feature is present without being asked for"
    );
    assert!(
        resolved_tauri_features(&["--features", "devtools"]).contains("devtools"),
        "the devtools feature flag did not reach tauri's resolved features"
    );
}

/// T6.32 — the release states its platform (P6.15.a, Requirements §2.1, §8).
#[test]
fn t6_32_the_release_states_its_platform() {
    let conf = std::fs::read_to_string(manifest_dir().join("tauri.conf.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&conf).unwrap();
    let long = json["bundle"]["longDescription"]
        .as_str()
        .expect("tauri.conf.json must set bundle.longDescription");
    let short = json["bundle"]["shortDescription"]
        .as_str()
        .expect("tauri.conf.json must set bundle.shortDescription");

    assert!(
        long.contains("macOS") || short.contains("macOS"),
        "the release's own description does not name macOS"
    );
    for forbidden in ["Windows", "Linux"] {
        assert!(
            !long.contains(forbidden) || long.contains("not"),
            "the release's description mentions {forbidden} without disclaiming it"
        );
    }

    // Bundling for another platform's format would itself be a claim of
    // support the release does not make.
    let targets = json["bundle"]["targets"]
        .as_array()
        .expect("tauri.conf.json must list explicit bundle targets, not \"all\"");
    let macos_only = ["app", "dmg", "updater"];
    for target in targets {
        let name = target.as_str().unwrap_or_default();
        assert!(
            macos_only.contains(&name),
            "bundle target {name:?} is not one of this release's macOS targets"
        );
    }
}

/// The `tauri` crate's resolved feature set for this package, via
/// `cargo metadata` rather than by parsing `Cargo.toml` — a feature can be
/// pulled in transitively, so what matters is what actually resolves.
fn resolved_tauri_features(extra_args: &[&str]) -> std::collections::HashSet<String> {
    let mut args = vec!["metadata", "--format-version", "1"];
    args.extend_from_slice(extra_args);
    let output = Command::new(env!("CARGO"))
        .args(&args)
        .current_dir(manifest_dir())
        .output()
        .expect("cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let nodes = metadata["resolve"]["nodes"].as_array().expect("nodes");
    for node in nodes {
        let id = node["id"].as_str().unwrap_or_default();
        // PackageId spec, e.g. "registry+https://…#tauri@2.11.5" — match the
        // part after `#` exactly, or "tauri-build"/"tauri-plugin-…" etc.
        // would match too.
        let name = id.rsplit('#').next().unwrap_or(id);
        if name.starts_with("tauri@") || name == "tauri" {
            return node["features"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|f| f.as_str().map(str::to_owned))
                .collect();
        }
    }
    panic!("tauri not found in resolved metadata");
}
