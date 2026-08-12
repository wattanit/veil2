//! Phase 7 test cases T7.5–T7.7 — the `--group` flag (FR-29).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use harness::Scratch;

/// T7.5 — `--group=extension` groups the listing, table and JSON.
#[test]
fn t7_5_group_by_extension_groups_the_listing() {
    let scratch = Scratch::new("group-extension");
    scratch.with_files(&[
        ("", "photo.jpg", "a"),
        ("", "photo2.JPG", "b"),
        ("", "notes.txt", "c"),
        ("", "README", "d"),
    ]);
    let vault = scratch.vault_arg();

    let table = scratch.veil(&["list", &vault, "--group=extension"]);
    assert_eq!(table.code, 0, "{}", table.everything());
    assert!(table.out.contains("\njpg\n"), "{}", table.out);
    assert!(table.out.contains("\ntxt\n"), "{}", table.out);
    assert!(table.out.contains("(no extension)"), "{}", table.out);
    assert!(table.out.contains("photo.jpg"), "{}", table.out);
    assert!(table.out.contains("photo2.JPG"), "{}", table.out);
    assert!(table.out.contains("README"), "{}", table.out);

    let json = scratch.veil(&["list", &vault, "--group=extension", "--format", "json"]);
    assert_eq!(json.code, 0, "{}", json.everything());
    let value: serde_json::Value = serde_json::from_str(&json.out).unwrap();
    let groups = value["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 3, "{value}"); // null, jpg, txt

    let jpg_group = groups
        .iter()
        .find(|g| g["group"] == "jpg")
        .expect("no jpg group in the JSON output");
    let jpg_names: Vec<&str> = jpg_group["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert_eq!(jpg_names.len(), 2, "{jpg_group}");

    let none_group = groups
        .iter()
        .find(|g| g["group"].is_null())
        .expect("no null (no-extension) group in the JSON output");
    let none_names: Vec<&str> = none_group["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert_eq!(none_names, vec!["README"]);
}

/// T7.6 — bare `--group`'s table output is unchanged; its JSON output is
/// not. Before this phase, `--format json` ignored `--group` entirely.
#[test]
fn t7_6_bare_group_table_unchanged_json_now_groups() {
    let scratch = Scratch::new("group-bare");
    scratch.with_files(&[("2024", "a.txt", "1"), ("2025", "b.txt", "2")]);
    let vault = scratch.vault_arg();

    let table = scratch.veil(&["list", &vault, "--group"]);
    assert_eq!(table.code, 0, "{}", table.everything());
    // The exact shape `output::grouped` produced before this phase: a
    // blank line, the folder heading, two-space-indented rows, a count.
    assert!(table.out.contains("\n2024\n"), "{}", table.out);
    assert!(table.out.contains("\n2025\n"), "{}", table.out);
    assert!(table.out.contains("  a.txt  "), "{}", table.out);
    assert!(table.out.contains("2 files"), "{}", table.out);

    let json = scratch.veil(&["list", &vault, "--group", "--format", "json"]);
    assert_eq!(json.code, 0, "{}", json.everything());
    let value: serde_json::Value = serde_json::from_str(&json.out).unwrap();
    let groups = value["groups"]
        .as_array()
        .expect("bare --group --format json did not group at all");
    assert_eq!(groups.len(), 2, "{value}");
}

/// T7.7 — omitted `--group` stays flat.
#[test]
fn t7_7_omitted_group_stays_flat() {
    let scratch = Scratch::new("group-omitted");
    scratch.with_files(&[("2024", "a.txt", "1"), ("2025", "b.txt", "2")]);
    let vault = scratch.vault_arg();

    let table = scratch.veil(&["list", &vault]);
    assert_eq!(table.code, 0, "{}", table.everything());
    assert!(!table.out.contains("\n2024\n"), "{}", table.out);

    let json = scratch.veil(&["list", &vault, "--format", "json"]);
    assert_eq!(json.code, 0, "{}", json.everything());
    let value: serde_json::Value = serde_json::from_str(&json.out).unwrap();
    assert!(value.get("files").is_some(), "{value}");
    assert!(value.get("groups").is_none(), "{value}");
}
