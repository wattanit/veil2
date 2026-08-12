//! Phase 8 test case T8.4 — column sort comparators (P8.2.b, P8.2.c, Design
//! §3.2).
//!
//! `ui/src/sort.ts` has no Rust counterpart to check agreement against, so
//! unlike `crates/veil-cli/tests/extension_parity.rs` (T7.4) this asserts
//! fixed expected output for a chosen fixture rather than parity between two
//! implementations. Bundled and run the identical way: the frontend's own
//! `esbuild`, then `node`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn ui_dir() -> PathBuf {
    manifest_dir().join("ui")
}

/// One fixture entry, as `sort.ts`'s comparators see it — the fields they
/// read (`name`, `folder`, `size`, `addedAt`), nothing else. Two rows (`b`,
/// `bb`) tie on `size` and `addedAt` to check stability (T8.4's own case).
const FIXTURE_JS: &str = r#"
[
  { id: 1, name: "Beta.txt",  folder: "Docs", size: 300, addedAt: 300 },
  { id: 2, name: "alpha.txt", folder: "docs", size: 100, addedAt: 100 },
  { id: 3, name: "b",         folder: "",     size: 200, addedAt: 200 },
  { id: 4, name: "bb",        folder: "",     size: 200, addedAt: 200 }
]
"#;

// `compareEntries` sorts descending by negating the ascending comparator,
// not by reversing the ascending-sorted array — for distinct keys the two
// are the same, but for a tie a stable sort keeps tied elements in their
// original relative order in *both* directions, since the comparator still
// returns 0 for them either way. So descending is the exact reverse of
// ascending only where this fixture's keys are distinct (name); where two
// rows tie (folder, size, added), the tied pair's own order stays fixed
// while the distinct groups around it reverse — which is what a person
// clicking a header twice should see: only the things that actually differ
// move.
#[test]
fn t8_4_comparators_sort_correctly_and_tie_stably_in_both_directions() {
    let result = run_typescript();

    // Name: no ties among these four names, so descending is a plain
    // reverse of ascending.
    assert_eq!(
        result.name_asc,
        vec!["alpha.txt", "b", "bb", "Beta.txt"],
        "name ascending should read case-insensitively"
    );
    assert_eq!(
        result.name_desc,
        vec!["Beta.txt", "bb", "b", "alpha.txt"],
        "name descending should be the exact reverse of ascending"
    );

    // Folder: "b"/"bb" (folder "") tie with each other; "Beta.txt"/"docs"
    // and "alpha.txt"/"docs" tie with each other. Each tied pair keeps its
    // original relative order (b before bb; Beta.txt before alpha.txt) in
    // both directions — only the two folders swap position.
    assert_eq!(result.folder_asc, vec!["b", "bb", "Beta.txt", "alpha.txt"]);
    assert_eq!(result.folder_desc, vec!["Beta.txt", "alpha.txt", "b", "bb"]);

    // Size: "b" and "bb" tie at 200 and keep their original order (b, bb)
    // regardless of direction; only their tied pair's position relative to
    // the distinct 100 and 300 rows reverses.
    assert_eq!(result.size_asc, vec!["alpha.txt", "b", "bb", "Beta.txt"]);
    assert_eq!(result.size_desc, vec!["Beta.txt", "b", "bb", "alpha.txt"]);

    // Added: this fixture ties the same two rows on `addedAt` as on `size`,
    // so the shape matches exactly.
    assert_eq!(result.added_asc, vec!["alpha.txt", "b", "bb", "Beta.txt"]);
    assert_eq!(result.added_desc, vec!["Beta.txt", "b", "bb", "alpha.txt"]);
}

#[derive(serde::Deserialize)]
struct Result_ {
    #[serde(rename = "nameAsc")]
    name_asc: Vec<String>,
    #[serde(rename = "nameDesc")]
    name_desc: Vec<String>,
    #[serde(rename = "folderAsc")]
    folder_asc: Vec<String>,
    #[serde(rename = "folderDesc")]
    folder_desc: Vec<String>,
    #[serde(rename = "sizeAsc")]
    size_asc: Vec<String>,
    #[serde(rename = "sizeDesc")]
    size_desc: Vec<String>,
    #[serde(rename = "addedAsc")]
    added_asc: Vec<String>,
    #[serde(rename = "addedDesc")]
    added_desc: Vec<String>,
}

/// Bundles `sort.ts` with the frontend's own `esbuild`, wraps it in a tiny
/// generated entry point that runs the fixture through every column and
/// direction, then runs the result under `node` and parses what it printed
/// — the same mechanism `extension_parity.rs` (T7.4) established.
fn run_typescript() -> Result_ {
    let ui_dir = ui_dir();
    let esbuild = ui_dir.join("node_modules/.bin/esbuild");
    assert!(
        esbuild.exists(),
        "{} is missing — run `npm install` in crates/veil-gui/ui first",
        esbuild.display()
    );

    let scratch = std::env::temp_dir().join(format!(
        "veil2-sort-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();

    let module_path = ui_dir.join("src/sort.ts");
    let entry = scratch.join("entry.ts");
    std::fs::write(
        &entry,
        format!(
            "import {{ sortEntries }} from {};\n\
             const fixture = {FIXTURE_JS};\n\
             function names(column, direction) {{\n\
             \treturn sortEntries(fixture, column, direction).map((e) => e.name);\n\
             }}\n\
             console.log(JSON.stringify({{\n\
             \tnameAsc: names(\"name\", \"asc\"),\n\
             \tnameDesc: names(\"name\", \"desc\"),\n\
             \tfolderAsc: names(\"folder\", \"asc\"),\n\
             \tfolderDesc: names(\"folder\", \"desc\"),\n\
             \tsizeAsc: names(\"size\", \"asc\"),\n\
             \tsizeDesc: names(\"size\", \"desc\"),\n\
             \taddedAsc: names(\"added\", \"asc\"),\n\
             \taddedDesc: names(\"added\", \"desc\"),\n\
             }}));\n",
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
    serde_json::from_str(stdout.trim()).unwrap()
}
