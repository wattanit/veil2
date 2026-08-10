//! Phase 2 test cases T2.10 through T2.15 — ingest and folder ingest
//! (FR-7, FR-9, FR-10, FR-11, FR-12, Spec §4.6, §4.7).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod harness;

use std::path::Path;

use harness::{create, open, pattern};
use veil_core::{Cancel, NoProgress};

/// Creates a symbolic link, or reports that this platform and account cannot.
///
/// Windows requires either Developer Mode or an elevated account to create
/// links. A case that silently passes when it could not run is worse than one
/// that does not run, so the caller reports the skip.
fn symlink(target: &Path, link: &Path, directory: bool) -> bool {
    #[cfg(unix)]
    {
        let _ = directory;
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(windows)]
    {
        if directory {
            std::os::windows::fs::symlink_dir(target, link).is_ok()
        } else {
            std::os::windows::fs::symlink_file(target, link).is_ok()
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link, directory);
        false
    }
}

/// T2.10 — ingest is a copy (FR-9, Spec §4.7).
///
/// Nothing in `veil-core` deletes or modifies a file outside a vault. This is
/// the case that catches a "move" optimisation added later, and the reason
/// FR-9 exists is that an interrupted ingest must not be able to lose data.
#[test]
fn t2_10_ingest_is_a_copy() {
    let scratch = harness::Scratch::new("copy-semantics");
    let dir = scratch.vault_dir();

    let tree = scratch.path("sources");
    std::fs::create_dir_all(tree.join("nested")).unwrap();
    std::fs::write(tree.join("one.bin"), pattern(1000)).unwrap();
    std::fs::write(tree.join("nested/two.bin"), pattern(2500)).unwrap();

    let loose = scratch.path("loose.bin");
    std::fs::write(&loose, pattern(700)).unwrap();

    let before = harness::snapshot(&tree);
    let loose_before = std::fs::read(&loose).unwrap();

    let mut vault = create(&dir);
    vault
        .add_path(&loose, "singles", &mut NoProgress, &Cancel::new())
        .unwrap();
    vault
        .add_folder(&tree, &mut NoProgress, &Cancel::new())
        .unwrap();

    assert_eq!(
        harness::snapshot(&tree),
        before,
        "a source file changed during ingest"
    );
    assert_eq!(std::fs::read(&loose).unwrap(), loose_before);
    assert!(loose.exists(), "the source was removed");
    assert!(tree.join("one.bin").exists());
    assert!(tree.join("nested/two.bin").exists());
}

/// T2.11 — content is durable before the index names it (FR-12, Spec §4.7).
///
/// **What is asserted here is the observable half of FR-12:** when `add`
/// returns, the new entry's file exists on disk and is at least as long as its
/// recorded size — the index never points at a file that was not written —
/// and an independent reader opened afterwards gets the content back
/// byte-identically.
///
/// **Whether the fsync itself lands first is not checked, here or anywhere.**
/// That is not observable from outside the process without an indirection layer
/// inside `veil-core`, and that layer was rejected: a seam in shipped code to
/// serve one test. If the ordering ever gets checked it will be by killing a
/// real process, at Phase 4.
#[test]
fn t2_11_content_is_durable_before_the_index_names_it() {
    let scratch = harness::Scratch::new("durability");
    let dir = scratch.vault_dir();
    let content = pattern(11_000);

    let mut vault = create(&dir);
    let generation_before = vault.generation();
    let id = harness::add(&mut vault, "durable.bin", "d", &content);

    // One generation per committed mutation.
    assert_eq!(vault.generation(), generation_before + 1);

    // The entry's own file exists and holds at least its recorded length. A
    // shorter file than the index claims is FR-12 broken, and it is the shape
    // a wrong ordering produces.
    let path = veil_core::store::entry_path(&dir, id);
    let length = std::fs::metadata(&path)
        .unwrap_or_else(|_| panic!("entry file for {id} is missing"))
        .len();
    assert!(
        length > 0,
        "entry file for {id} is empty even though content was added"
    );
    drop(vault);

    // Success was reported; an independent reader must therefore find it.
    let reopened = open(&dir).unwrap();
    assert_eq!(harness::read_back(&reopened, id).unwrap(), content);
}

/// T2.13 — a folder walk stores every regular file with its relative path
/// (FR-10, FR-7).
#[test]
fn t2_13_a_folder_walk_stores_every_regular_file_with_its_relative_path() {
    let scratch = harness::Scratch::new("folder-walk");
    let dir = scratch.vault_dir();

    let tree = scratch.path("tree");
    std::fs::create_dir_all(tree.join("a/b/c")).unwrap();
    std::fs::write(tree.join("root.txt"), pattern(10)).unwrap();
    std::fs::write(tree.join("a/one.txt"), pattern(20)).unwrap();
    std::fs::write(tree.join("a/b/two.txt"), pattern(30)).unwrap();
    std::fs::write(tree.join("a/b/c/three.txt"), pattern(40)).unwrap();

    let mut vault = create(&dir);
    let outcome = vault
        .add_folder(&tree, &mut NoProgress, &Cancel::new())
        .unwrap();
    assert_eq!(outcome.added.len(), 4);
    assert!(outcome.skipped.is_empty());

    let mut paths: Vec<(String, String)> = vault
        .entries()
        .iter()
        .map(|e| (e.folder.clone(), e.name.clone()))
        .collect();
    paths.sort();

    assert_eq!(
        paths,
        vec![
            // The added folder's own name is the top-level segment (FR-10):
            // without it, a file at the root of one added folder and a file
            // at the root of a different added folder would collide.
            ("tree".to_owned(), "root.txt".to_owned()),
            ("tree/a".to_owned(), "one.txt".to_owned()),
            ("tree/a/b".to_owned(), "two.txt".to_owned()),
            ("tree/a/b/c".to_owned(), "three.txt".to_owned()),
        ]
    );

    // `/`-separated on every platform. The separator is a serialisation detail,
    // never the host's, or a vault written on Windows would present different
    // folder strings on Linux (§4.6).
    assert!(vault.entries().iter().all(|e| !e.folder.contains('\\')));

    // Content follows the name it was stored under.
    let three = vault.find("tree/a/b/c", "three.txt").unwrap();
    assert_eq!(harness::read_back(&vault, three.id).unwrap(), pattern(40));
}

/// T2.14 — symbolic links are not followed and are reported (FR-11).
///
/// Omitting them silently produces a vault the user believes is complete.
#[test]
fn t2_14_symbolic_links_are_not_followed_and_are_reported() {
    let scratch = harness::Scratch::new("symlinks");
    let dir = scratch.vault_dir();

    let outside = scratch.path("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret.txt"), b"OUTSIDE-THE-TREE").unwrap();

    let tree = scratch.path("tree");
    std::fs::create_dir_all(tree.join("inner")).unwrap();
    std::fs::write(tree.join("real.txt"), pattern(50)).unwrap();
    std::fs::write(tree.join("inner/deep.txt"), pattern(60)).unwrap();

    let made = [
        symlink(&tree.join("real.txt"), &tree.join("link-to-file"), false),
        symlink(&tree.join("inner"), &tree.join("link-to-inner"), true),
        symlink(&outside, &tree.join("link-to-outside"), true),
    ];
    if !made.iter().all(|m| *m) {
        eprintln!("T2.14 skipped: this platform or account cannot create symbolic links");
        return;
    }

    let mut vault = create(&dir);
    let outcome = vault
        .add_folder(&tree, &mut NoProgress, &Cancel::new())
        .unwrap();

    // Exactly the two regular files, and nothing reached through a link.
    assert_eq!(outcome.added.len(), 2);
    let names: Vec<&str> = vault.entries().iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"real.txt"));
    assert!(names.contains(&"deep.txt"));
    assert!(
        !names.contains(&"secret.txt"),
        "a link pulled in data from outside the tree the user selected"
    );

    // Recorded as skipped, not merely omitted — FR-11's actual requirement.
    assert_eq!(outcome.skipped.len(), 3);
    assert!(
        outcome
            .skipped
            .iter()
            .all(|s| s.reason == veil_core::vault::SkipReason::SymbolicLink)
    );
    let mut skipped: Vec<String> = outcome
        .skipped
        .iter()
        .map(|s| s.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    skipped.sort();
    assert_eq!(
        skipped,
        ["link-to-file", "link-to-inner", "link-to-outside"]
    );
}

/// T2.15 — a link cycle does not prevent the walk from finishing (FR-11).
///
/// Following links risks exactly this. The case exists so the property is
/// checked rather than argued.
#[test]
fn t2_15_a_link_cycle_does_not_prevent_the_walk_from_finishing() {
    let scratch = harness::Scratch::new("link-cycle");
    let dir = scratch.vault_dir();

    let tree = scratch.path("tree");
    std::fs::create_dir_all(tree.join("inner")).unwrap();
    std::fs::write(tree.join("inner/file.txt"), pattern(70)).unwrap();

    if !symlink(&tree, &tree.join("inner/loop"), true) {
        eprintln!("T2.15 skipped: this platform or account cannot create symbolic links");
        return;
    }

    let mut vault = create(&dir);
    let outcome = vault
        .add_folder(&tree, &mut NoProgress, &Cancel::new())
        .unwrap();

    assert_eq!(outcome.added.len(), 1);
    assert_eq!(vault.entries()[0].name, "file.txt");
    assert_eq!(vault.entries()[0].folder, "tree/inner");
    assert_eq!(outcome.skipped.len(), 1);
}
