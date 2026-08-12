//! Phase 5 test cases T5.5, T5.6, and T5.7, Phase 6's T6.32, Phase 7's
//! T7.15, and Phase 8's T8.21 — properties of the shell's configuration
//! and source structure rather than of behaviour (Spec §5.3, HC-1,
//! Requirements §2.1, §8, Design §8.10).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::process::Command;

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// T5.5 — the Content-Security-Policy admits no remote origin (P5.3.a,
/// Spec §5.3, §7).
///
/// `blob:` was added to `img-src` in P8.6's fix for a release-build defect:
/// preview's image renders from `URL.createObjectURL`, which a release
/// build's stricter CSP enforcement was blocking outright (silently — no
/// thrown error, just no image), while a `cargo tauri dev` run let it
/// through. `blob:` is not a remote origin — it names an ephemeral,
/// webview-local reference to an in-memory `Blob`, the same class of thing
/// `data:` already is here — so admitting it does not weaken what this
/// case actually checks.
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
                || source == "blob:"
                || source == "ipc:"
                || source == "http://ipc.localhost";
            assert!(
                allowed,
                "CSP directive {directive:?} names a source ({source}) that is \
                 neither the bundle, a local in-memory reference, nor Tauri's own IPC channel"
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

/// T7.15 (P7.7.b) — no `tracing` call, and no `tracing` dependency, exists
/// anywhere in this crate.
///
/// `preview_entry` is the first place this crate handles new plaintext
/// (Requirements C-5), so P7.7.b asks for the same "no log line names an
/// operation's content" discipline every other command holds to. Read
/// literally, though, no command here holds to that discipline by
/// *logging carefully* — none of them logs at all. `veil-gui` carries no
/// `tracing` dependency (only `veil-core`'s does, for its own guard,
/// `tests/logging_guard.rs`, which cannot reach this crate's commands).
/// So the guarantee is proved by construction, the same class of check
/// T5.6 already makes for persistent storage APIs: absence, not careful
/// use.
#[test]
fn t7_15_no_tracing_dependency_or_call_exists_in_veil_gui() {
    let manifest = std::fs::read_to_string(manifest_dir().join("Cargo.toml")).unwrap();
    assert!(
        !manifest.contains("tracing"),
        "veil-gui/Cargo.toml now depends on tracing — P7.7.b's \"proved by \
         construction\" no longer holds; either instrument preview_entry \
         (and audit what it logs) or keep this crate free of the dependency"
    );

    let mut offending = Vec::new();
    let mut stack = vec![manifest_dir().join("src")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if std::fs::read_to_string(&path).unwrap().contains("tracing") {
                offending.push(path.display().to_string());
            }
        }
    }
    assert!(
        offending.is_empty(),
        "tracing is referenced in this crate's own source, which the \
         Cargo.toml check above did not expect:\n  {}",
        offending.join("\n  ")
    );
}

/// T8.21 (P8.7.c) — quitting reuses `lock()`'s own clearing calls, not a
/// second implementation written for `beforeunload`.
///
/// A source-and-manifest scan, the same class of check T5.6 and T7.15
/// already make: `closeContextMenu`/`closeDetails`/`closePreview` are each
/// defined exactly once, and both `lock()` and the `beforeunload` handler
/// call all three — proving the two call sites share one routine rather
/// than each composing its own.
#[test]
fn t8_21_quitting_reuses_locks_own_clearing_calls_not_a_second_implementation() {
    let source =
        std::fs::read_to_string(manifest_dir().join("ui").join("src").join("main.ts")).unwrap();

    for name in ["closeContextMenu", "closeDetails", "closePreview"] {
        let definitions = source.matches(&format!("function {name}(")).count();
        assert_eq!(
            definitions, 1,
            "{name} should be defined exactly once, not duplicated for a second call site"
        );
    }

    let lock_body = between(&source, "async function lock(): Promise<void> {", "\n}");
    let unload_body = between(&source, "\"beforeunload\", () => {", "\n  });");

    for name in ["closeContextMenu", "closeDetails", "closePreview"] {
        let call = format!("{name}();");
        assert!(
            lock_body.contains(&call),
            "lock() should call {call} to clear preview/details state on lock (Design §8.10, FR-3)"
        );
        assert!(
            unload_body.contains(&call),
            "the beforeunload handler should call {call}, the same routine lock() uses"
        );
    }
}

/// The text between the end of `start` and the next occurrence of `end` —
/// enough to isolate one function's body from the rest of the file
/// without a full parser.
fn between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_idx = source.find(start).expect("start marker not found") + start.len();
    let rest = &source[start_idx..];
    let end_idx = rest.find(end).expect("end marker not found");
    &rest[..end_idx]
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
