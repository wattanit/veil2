//! Phase 8 test case T8.23 — the shared damaged-file message (P8.8.a,
//! Design §6, §8.10).
//!
//! `ui/src/damage.ts` has no Rust counterpart, so — like `sort.rs` (T8.4)
//! and `selection.rs` (T8.7) — this checks fixed expected output rather
//! than parity between two implementations. Bundled and run the same way:
//! the frontend's own `esbuild`, then `node`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn ui_dir() -> PathBuf {
    manifest_dir().join("ui")
}

#[test]
fn t8_23_the_damaged_file_message_names_a_removed_copy_only_for_extraction() {
    let (with_copy, without_copy) = run_typescript();

    assert_eq!(
        with_copy,
        "report.pdf is damaged in the vault. The incomplete copy has been removed. \
         The vault's other files are unaffected."
    );
    assert_eq!(
        without_copy,
        "report.pdf is damaged in the vault. The vault's other files are unaffected."
    );

    // The two calls differ in exactly the one clause that is true for
    // extraction and untrue for preview — everything either side actually
    // asserts about the failure itself is identical, which is what Design
    // §8.10's "worded exactly as an extraction failure" asks for.
    assert!(with_copy.starts_with("report.pdf is damaged in the vault."));
    assert!(without_copy.starts_with("report.pdf is damaged in the vault."));
    assert!(with_copy.ends_with("The vault's other files are unaffected."));
    assert!(without_copy.ends_with("The vault's other files are unaffected."));
}

/// Bundles `damage.ts` with the frontend's own `esbuild`, calls
/// `damagedFileMessage` with `removedCopy` both `true` and `false`, then
/// runs the result under `node` — the same mechanism `sort.rs` and
/// `selection.rs` established.
fn run_typescript() -> (String, String) {
    let ui_dir = ui_dir();
    let esbuild = ui_dir.join("node_modules/.bin/esbuild");
    assert!(
        esbuild.exists(),
        "{} is missing — run `npm install` in crates/veil-gui/ui first",
        esbuild.display()
    );

    let scratch = std::env::temp_dir().join(format!(
        "veil2-damage-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();

    let module_path = ui_dir.join("src/damage.ts");
    let entry = scratch.join("entry.ts");
    std::fs::write(
        &entry,
        format!(
            "import {{ damagedFileMessage }} from {};\n\
             console.log(JSON.stringify([\n\
             \tdamagedFileMessage(\"report.pdf\", true),\n\
             \tdamagedFileMessage(\"report.pdf\", false),\n\
             ]));\n",
            serde_json::to_string(&module_path.to_string_lossy()).unwrap(),
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
    let pair: Vec<String> = serde_json::from_str(stdout.trim()).unwrap();
    (pair[0].clone(), pair[1].clone())
}
