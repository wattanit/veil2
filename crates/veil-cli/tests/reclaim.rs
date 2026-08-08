//! Phase 4 test cases T4.15 and T4.16 — reclaiming space from the command
//! line (Design §7, §8.4; FR-8, FR-21, FR-22, FR-23, FR-29).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use harness::{Scratch, run};

/// T4.15 — reclaiming has no schedule and no condition (FR-23, Spec §5.2).
///
/// The same verdict as T3.3, re-asserted now that the operation FR-23 is
/// actually about exists. Until this phase there was nothing for those flags to
/// schedule, so FR-23 was being asserted against an absence.
#[test]
fn t4_15_reclaiming_has_no_schedule_and_no_condition() {
    const FORBIDDEN: [&str; 10] = [
        "--schedule",
        "--daemon",
        "--watch",
        "--interval",
        "--cron",
        "--every",
        "--if-reclaimable",
        "--threshold",
        "--auto",
        "--when",
    ];

    let help = run(&["reclaim-space", "--help"]).everything();
    assert!(
        help.contains("reclaim"),
        "the command is not there to check: {help}"
    );
    for flag in FORBIDDEN {
        assert!(!help.contains(flag), "reclaim-space carries {flag}");
    }

    // And the words Design §7 forbids for this operation, at the one place the
    // implementation's own vocabulary is most likely to leak.
    for word in ["compact", "vacuum", "garbage", "gc"] {
        assert!(
            !help.to_lowercase().contains(word),
            "reclaim-space says \"{word}\", which Design §7 forbids"
        );
    }
}

/// T4.16 — the command says what it did, and delete no longer says what is no
/// longer true (FR-8, FR-21, FR-22, FR-29, Design §7, §8.4).
///
/// A true sentence in Phase 3 — *this version cannot reclaim space* — becomes a
/// false one here. That kind of message survives for years, because nothing
/// tests prose.
#[test]
fn t4_16_the_command_says_what_it_did() {
    let scratch = Scratch::new("reclaim-says");
    let vault = scratch.vault_arg();
    scratch.with_files(&[
        ("docs", "keep.txt", "content that stays in the vault"),
        ("docs", "drop.txt", "content that will be deleted shortly"),
    ]);

    let deleted = scratch.veil(&["delete", &vault, "docs/drop.txt"]);
    assert_eq!(deleted.code, 0);
    let said = deleted.everything().to_lowercase();
    assert!(
        said.contains("until you reclaim space"),
        "delete no longer states that the bytes remain: {said}"
    );
    assert!(
        !said.contains("cannot reclaim space yet"),
        "delete still says this version cannot reclaim space: {said}"
    );

    // The figures the decision rests on are in front of the person making it.
    let before: serde_json::Value =
        serde_json::from_str(&scratch.veil(&["info", &vault, "--format", "json"]).out).unwrap();
    let reclaimable = before["reclaimable_bytes"].as_u64().unwrap();
    assert!(reclaimable > 0, "there is nothing to reclaim");

    let done = scratch.veil(&["reclaim-space", &vault]);
    assert_eq!(done.code, 0, "{}", done.err);
    let out = done.out.to_lowercase();
    assert!(
        out.contains("reclaimed"),
        "the result does not say what it did: {out}"
    );
    assert!(
        out.contains("gone from the vault now"),
        "nothing said the deleted bytes are now actually gone: {out}"
    );

    // Machine mode carries the same facts, exactly (Design §3.4).
    scratch.veil(&[
        "add",
        &vault,
        scratch.write("more.txt", "x").to_str().unwrap(),
    ]);
    assert_eq!(
        scratch.veil(&["delete", &vault, "more.txt"]).code,
        0,
        "the second delete failed"
    );
    let json: serde_json::Value = serde_json::from_str(
        &scratch
            .veil(&["reclaim-space", &vault, "--format", "json"])
            .out,
    )
    .unwrap();
    assert!(json["bytes_recovered"].as_u64().unwrap() > 0);
    assert_eq!(json["complete"], true);
    assert_eq!(json["reclaimable_bytes"], 0);

    // And the figures agree with the vault afterwards.
    let after: serde_json::Value =
        serde_json::from_str(&scratch.veil(&["info", &vault, "--format", "json"]).out).unwrap();
    assert_eq!(after["reclaimable_bytes"], 0);
    assert_eq!(after["files"], 1);

    // The file that was kept is still exactly what it was.
    let saved = scratch.path("keep.txt");
    assert_eq!(
        scratch
            .veil(&[
                "save-copy",
                &vault,
                "docs/keep.txt",
                "--to",
                saved.to_str().unwrap()
            ])
            .code,
        0
    );
    assert_eq!(
        std::fs::read_to_string(&saved).unwrap(),
        "content that stays in the vault"
    );
}
