//! Phase 7 test cases T7.1–T7.3 — per-entry detail (FR-28).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use harness::Scratch;

/// T7.1 — `veil detail` reports full metadata.
#[test]
fn t7_1_detail_reports_full_metadata() {
    let scratch = Scratch::new("detail-table");
    scratch.with_files(&[("work/2024", "report.pdf", "the content")]);
    let vault = scratch.vault_arg();

    let run = scratch.veil(&["detail", &vault, "work/2024/report.pdf"]);
    assert_eq!(run.code, 0, "{}", run.everything());
    assert!(run.out.contains("report.pdf"), "{}", run.out);
    assert!(run.out.contains("work/2024"), "{}", run.out);
    assert!(run.out.contains("Modified"), "{}", run.out);
    assert!(run.out.contains("Added"), "{}", run.out);
    assert!(
        !run.out.to_lowercase().contains("hash"),
        "the detail view showed an internal hash: {}",
        run.out
    );
}

/// T7.2 — `detail`'s JSON carries the same facts.
#[test]
fn t7_2_detail_json_carries_the_same_facts() {
    let scratch = Scratch::new("detail-json");
    scratch.with_files(&[("", "notes.txt", "hello")]);
    let vault = scratch.vault_arg();

    let run = scratch.veil(&["detail", &vault, "notes.txt", "--format", "json"]);
    assert_eq!(run.code, 0, "{}", run.everything());

    let value: serde_json::Value = serde_json::from_str(&run.out).unwrap();
    assert_eq!(value["name"], "notes.txt");
    assert_eq!(value["folder"], "");
    assert_eq!(value["size"], 5);
    assert!(value.get("modified").is_some(), "{value}");
    assert!(value.get("added").is_some(), "{value}");
    assert!(
        value.get("hash").is_none(),
        "the JSON exposed a hash field: {value}"
    );
}

/// T7.3 — `detail` on an unknown path names nothing, not damage.
#[test]
fn t7_3_detail_on_an_unknown_path_is_not_damage() {
    let scratch = Scratch::new("detail-missing");
    scratch.with_files(&[("docs", "real.txt", "content")]);
    let vault = scratch.vault_arg();

    let run = scratch.veil(&["detail", &vault, "docs/ghost.txt"]);
    assert_eq!(run.code, 13, "{}", run.everything());
    assert!(run.err.contains("docs/ghost.txt"), "{}", run.err);
    assert!(
        !run.err.contains("damage") && !run.err.contains("damaged"),
        "a mistyped name was reported as damage: {}",
        run.err
    );
}
