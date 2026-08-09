//! Phase 3 test cases T3.8–T3.12 — what the commands write
//! (Design §3.4, §7; FR-7, FR-8).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use harness::Scratch;

/// T3.8 — the table is in the fixed column order, and the filters filter.
#[test]
fn t3_8_the_table_is_in_the_fixed_column_order() {
    let scratch = Scratch::new("columns");
    let vault = scratch.vault_arg();
    scratch.with_files(&[
        ("work/2024", "report.pdf", "a document"),
        ("work/2025", "budget.csv", "some numbers"),
        ("photos", "sunset.jpg", "an image"),
    ]);

    let listed = scratch.veil(&["list", &vault]);
    assert_eq!(listed.code, 0, "{}", listed.err);

    let header = listed.out.lines().next().unwrap();
    let order: Vec<usize> = ["Name", "Folder", "Size", "Added"]
        .iter()
        .map(|column| {
            header
                .find(column)
                .unwrap_or_else(|| panic!("the table has no {column} column: {header}"))
        })
        .collect();
    assert!(
        order.windows(2).all(|w| w[0] < w[1]),
        "the columns are not in Design's order: {header}"
    );

    // Counts are exact and never rounded (Design §7).
    assert!(listed.out.contains("3 files"), "{}", listed.out);

    let by_folder = scratch.veil(&["list", &vault, "--folder", "work"]);
    assert!(by_folder.out.contains("report.pdf") && by_folder.out.contains("budget.csv"));
    assert!(!by_folder.out.contains("sunset.jpg"), "{}", by_folder.out);
    assert!(by_folder.out.contains("2 files"));

    let by_name = scratch.veil(&["list", &vault, "--name", "sun"]);
    assert!(by_name.out.contains("sunset.jpg"));
    assert!(!by_name.out.contains("report.pdf"), "{}", by_name.out);

    let grouped = scratch.veil(&["list", &vault, "--group"]);
    assert!(grouped.out.contains("work/2024"), "{}", grouped.out);
    assert!(grouped.out.contains("photos"), "{}", grouped.out);
}

/// T3.9 — stored names are printed exactly, in every script.
///
/// Column alignment for double-width scripts is deliberately not asserted:
/// a test that demanded it would push the implementation toward padding
/// names, which is not something this product does to a stored name.
#[test]
fn t3_9_names_come_back_exactly_as_stored() {
    const NAMES: [&str; 5] = [
        "report.pdf",
        "รายงานประจำปี.pdf",
        "تقرير-سنوي.pdf",
        "年度報告書.pdf",
        "🎂-birthday.jpg",
    ];

    let scratch = Scratch::new("scripts");
    let vault = scratch.vault_arg();
    scratch.with_files(
        &NAMES
            .iter()
            .map(|name| ("docs", *name, "content"))
            .collect::<Vec<_>>(),
    );

    let listed = scratch.veil(&["list", &vault]);
    assert_eq!(listed.code, 0, "{}", listed.err);
    for name in NAMES {
        assert!(
            listed.out.contains(name),
            "{name} did not come back as stored:\n{}",
            listed.out
        );
    }

    // And a name in a non-Latin script is still addressable.
    let saved = scratch.path("out.pdf");
    let run = scratch.veil(&[
        "save-copy",
        &vault,
        "docs/年度報告書.pdf",
        "--to",
        saved.to_str().unwrap(),
    ]);
    assert_eq!(run.code, 0, "{}", run.everything());
}

/// T3.10 — machine output carries the same facts, in machine form.
#[test]
fn t3_10_machine_output_carries_the_same_facts() {
    let scratch = Scratch::new("json");
    let vault = scratch.vault_arg();
    scratch.with_files(&[("docs", "report.pdf", "0123456789")]);

    let listed = scratch.veil(&["list", &vault, "--format", "json"]);
    assert_eq!(listed.code, 0, "{}", listed.err);
    let parsed: serde_json::Value = serde_json::from_str(&listed.out).unwrap();
    let file = &parsed["files"][0];
    assert_eq!(file["name"], "report.pdf");
    assert_eq!(file["folder"], "docs");
    // Exact bytes, not "10 B" — the whole point of the mode.
    assert_eq!(file["size"], 10);
    assert!(file["added"].as_u64().unwrap() > 0);

    let info = scratch.veil(&["info", &vault, "--format", "json"]);
    let stats: serde_json::Value = serde_json::from_str(&info.out).unwrap();
    assert_eq!(stats["files"], 1);
    assert_eq!(stats["logical_bytes"], 10);

    // A failure in machine mode is reported in machine mode. A script that
    // meets prose only when something goes wrong has no error handling.
    let failed = scratch.veil(&["delete", &vault, "docs/ghost.pdf", "--format", "json"]);
    assert_eq!(failed.code, 13);
    let reported: serde_json::Value = serde_json::from_str(&failed.err).unwrap();
    assert_eq!(reported["exit_code"], 13);
    assert!(reported["error"].as_str().unwrap().contains("ghost.pdf"));
}

/// T3.11 — the streams stay separated.
#[test]
fn t3_11_standard_output_carries_results_alone() {
    let scratch = Scratch::new("streams");
    let vault = scratch.vault_arg();
    scratch.with_files(&[("docs", "a.txt", "one"), ("docs", "b.txt", "two")]);

    let listed = scratch.veil(&["list", &vault, "--format", "json"]);
    assert_eq!(listed.code, 0);
    // Valid from the first byte to the last: no banner, no progress, no prose.
    serde_json::from_str::<serde_json::Value>(&listed.out).unwrap_or_else(|e| {
        panic!(
            "standard output was not clean machine output: {e}\n{}",
            listed.out
        )
    });

    // The human default is never machine-shaped.
    let table = scratch.veil(&["list", &vault]);
    assert!(
        serde_json::from_str::<serde_json::Value>(&table.out).is_err(),
        "the human default emitted machine output"
    );
    assert!(table.out.contains("Name"), "{}", table.out);
}

/// T3.12 — the reported statistics are the vault's (FR-7).
#[test]
fn t3_12_the_statistics_are_the_vaults() {
    let scratch = Scratch::new("statistics");
    let vault = scratch.vault_arg();
    scratch.with_files(&[
        ("docs", "a.txt", "one hundred bytes or so of content here"),
        ("docs", "b.txt", "another file"),
    ]);

    let before: serde_json::Value =
        serde_json::from_str(&scratch.veil(&["info", &vault, "--format", "json"]).out).unwrap();
    assert_eq!(before["files"], 2);

    assert_eq!(scratch.veil(&["delete", &vault, "docs/b.txt"]).code, 0);

    let after: serde_json::Value =
        serde_json::from_str(&scratch.veil(&["info", &vault, "--format", "json"]).out).unwrap();
    assert_eq!(after["files"], 1);
    assert_eq!(
        after["logical_bytes"].as_u64().unwrap(),
        "one hundred bytes or so of content here".len() as u64
    );
    // No reclaimable figure is printed — there is nothing left to reclaim
    // once delete has already freed the file.
    assert!(after.get("reclaimable_bytes").is_none());

    // And the same figures the library reports for the same vault (FR-7).
    let opened = veil_core::vault::Vault::open(
        &scratch.vault(),
        &veil_core::crypto::Password::new(harness::PASSWORD.to_owned()),
    )
    .unwrap();
    let stats = opened.statistics();
    assert_eq!(after["files"].as_u64().unwrap(), stats.entry_count);
    assert_eq!(
        after["logical_bytes"].as_u64().unwrap(),
        stats.logical_bytes
    );
}
