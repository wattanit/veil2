//! Phase 7 test case T7.4 — extension derivation (FR-29).
//!
//! Checks this crate's `extension_of` and the GUI frontend's `extensionOf`
//! (`crates/veil-gui/ui/src/extension.ts`) against one shared fixture list,
//! so the two independently-written implementations are proven to agree
//! rather than assumed to (Tech Spec §5.1, §9).
//!
//! The TypeScript side is transpiled with the frontend's own `esbuild`
//! (already a devDependency of `veil-gui/ui`, already installed for its
//! `npm run build`) rather than run through whatever TypeScript support the
//! local `node` happens to have built in — a choice pinned to the tool this
//! repository already carries, per the ToDo's note on why (the same
//! proportionality call Phase 5 made declining a WebDriver suite).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::process::Command;

use veil_cli::extension::extension_of;

/// The one shared list (P7.2.c) — every case Phase7-TestCases.md's T7.4
/// documents, and nothing else. Extending it here is extending the
/// requirement, not just the test.
const FIXTURES: &[(&str, Option<&str>)] = &[
    ("archive.tar.gz", Some("gz")),
    (".gitignore", None),
    ("README", None),
    ("photo.JPG", Some("jpg")),
    ("file.", None),
    ("a.b.c", Some("c")),
    ("IMG_1.png", Some("png")),
];

#[test]
fn t7_4_both_implementations_agree_on_the_written_rule() {
    for (name, expected) in FIXTURES.iter().copied() {
        assert_eq!(extension_of(name).as_deref(), expected, "Rust: {name:?}");
    }

    let names: Vec<&str> = FIXTURES.iter().map(|(name, _)| *name).collect();
    let ts_results = run_typescript(&names);
    assert_eq!(
        ts_results.len(),
        FIXTURES.len(),
        "TypeScript returned {} answers for {} names",
        ts_results.len(),
        FIXTURES.len()
    );

    for ((name, expected), actual) in FIXTURES.iter().copied().zip(&ts_results) {
        assert_eq!(actual.as_deref(), expected, "TypeScript: {name:?}");
    }
}

/// Transpiles `extension.ts` with the frontend's own `esbuild`, bundling it
/// with a tiny generated entry point, then runs the result under `node`
/// against every name and parses what it printed.
fn run_typescript(names: &[&str]) -> Vec<Option<String>> {
    let ui_dir = ui_dir();
    let esbuild = ui_dir.join("node_modules/.bin/esbuild");
    assert!(
        esbuild.exists(),
        "{} is missing — run `npm install` in crates/veil-gui/ui first",
        esbuild.display()
    );

    let scratch = std::env::temp_dir().join(format!(
        "veil2-extension-parity-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();

    // `--bundle` inlines extension.ts's own content, so the result is one
    // self-contained file with no runtime module resolution left to get
    // wrong (Node ESM resolving an absolute .ts path is not something to
    // depend on).
    let module_path = ui_dir.join("src/extension.ts");
    let entry = scratch.join("entry.ts");
    std::fs::write(
        &entry,
        format!(
            "import {{ extensionOf }} from {};\n\
             const names = {};\n\
             console.log(JSON.stringify(names.map(extensionOf)));\n",
            serde_json::to_string(&module_path.to_string_lossy()).unwrap(),
            serde_json::to_string(&names).unwrap(),
        ),
    )
    .unwrap();

    let bundled = scratch.join("entry.mjs");
    let build = Command::new(&esbuild)
        .arg(&entry)
        .arg("--bundle")
        .arg("--platform=node")
        .arg("--format=esm")
        .arg(format!("--outfile={}", bundled.display()))
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "esbuild failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new("node").arg(&bundled).output().unwrap();
    assert!(
        run.status.success(),
        "node failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    let _ = std::fs::remove_dir_all(&scratch);

    let stdout = String::from_utf8_lossy(&run.stdout);
    serde_json::from_str(stdout.trim()).unwrap()
}

fn ui_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("veil-gui/ui")
}
