//! Phase 8 test case T8.7 — multi-select transitions (P8.3.a, P8.3.b, Design
//! §3.2).
//!
//! `ui/src/selection.ts` has no Rust counterpart, so — like `sort.rs`
//! (T8.4) — this checks fixed expected output for a chosen click sequence
//! rather than parity between two implementations. Bundled and run the same
//! way: the frontend's own `esbuild`, then `node`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn ui_dir() -> PathBuf {
    manifest_dir().join("ui")
}

#[derive(serde::Deserialize)]
struct StepResult {
    #[serde(rename = "selectedIds")]
    selected_ids: Vec<i64>,
    #[serde(rename = "lastClickedId")]
    last_clicked_id: Option<i64>,
}

/// Phase8-TestCases.md's own T8.7 sequence, against a visual order of rows
/// 1 through 10: plain click on 3; shift-click on 7 (3-7 selected); Cmd-click
/// on 5 (5 removed, 3-4 and 6-7 remain); plain click on 1 (only 1 selected).
#[test]
fn t8_7_selection_transitions_match_each_click_kind() {
    let steps = run_typescript();
    assert_eq!(steps.len(), 4);

    let mut sorted = steps[0].selected_ids.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, vec![3], "a plain click should select only that row");
    assert_eq!(steps[0].last_clicked_id, Some(3));

    let mut sorted = steps[1].selected_ids.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        vec![3, 4, 5, 6, 7],
        "a shift-click should extend a contiguous range from the anchor"
    );
    assert_eq!(
        steps[1].last_clicked_id,
        Some(3),
        "the anchor should stay at the plain-clicked row, not move to the shift-clicked one"
    );

    let mut sorted = steps[2].selected_ids.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        vec![3, 4, 6, 7],
        "a Cmd-click should remove exactly the clicked row, leaving the rest of the range"
    );
    assert_eq!(steps[2].last_clicked_id, Some(5));

    let mut sorted = steps[3].selected_ids.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        vec![1],
        "a plain click after a multi-selection should replace it entirely"
    );
    assert_eq!(steps[3].last_clicked_id, Some(1));
}

/// Bundles `selection.ts` with the frontend's own `esbuild`, wraps it in a
/// generated entry point that replays the click sequence against a visual
/// order of `[1..=10]`, then runs the result under `node` — the same
/// mechanism `extension_parity.rs` (T7.4) and `sort.rs` (T8.4) established.
fn run_typescript() -> Vec<StepResult> {
    let ui_dir = ui_dir();
    let esbuild = ui_dir.join("node_modules/.bin/esbuild");
    assert!(
        esbuild.exists(),
        "{} is missing — run `npm install` in crates/veil-gui/ui first",
        esbuild.display()
    );

    let scratch = std::env::temp_dir().join(format!(
        "veil2-selection-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();

    let module_path = ui_dir.join("src/selection.ts");
    let entry = scratch.join("entry.ts");
    std::fs::write(
        &entry,
        format!(
            "import {{ nextSelection }} from {};\n\
             const visualOrder = [1,2,3,4,5,6,7,8,9,10];\n\
             let state = {{ selectedIds: new Set(), lastClickedId: null }};\n\
             const steps = [];\n\
             function click(id, kind) {{\n\
             \tstate = nextSelection(state, visualOrder, id, kind);\n\
             \tsteps.push({{\n\
             \t\tselectedIds: [...state.selectedIds],\n\
             \t\tlastClickedId: state.lastClickedId,\n\
             \t}});\n\
             }}\n\
             click(3, \"plain\");\n\
             click(7, \"shift\");\n\
             click(5, \"cmd\");\n\
             click(1, \"plain\");\n\
             console.log(JSON.stringify(steps));\n",
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
