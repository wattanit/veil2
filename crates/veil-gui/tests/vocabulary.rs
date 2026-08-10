//! Phase 6 test case T6.31 — the vocabulary audit (Design §1.2, §7).
//!
//! Extracts string literals from the frontend's own source and from the
//! CLI's, and checks them against Design §7's fixed-vocabulary denylist and
//! forbidden-claims list, and §1.2's anti-goal vocabulary. A mechanical
//! check, not a parser: it finds quoted text, which is where user-facing
//! strings live in both languages, and accepts a short, explicit allowlist
//! for identifiers that are not prose (event names, CSS class names, mode
//! strings) rather than trying to distinguish those structurally.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

/// (banned word or phrase, why — the vocabulary table's "Use" column, or
/// the anti-goal/forbidden-claim it stands in for).
const DENYLIST: &[(&str, &str)] = &[
    ("entry", "file"),
    ("object", "file"),
    ("blob", "file"),
    ("container", "vault"),
    ("archive", "vault"),
    ("repository", "vault"),
    ("passphrase", "password"),
    ("master password", "password"),
    ("ingest", "add"),
    ("decrypt", "save a copy"),
    ("download", "save a copy"),
    ("integrity check", "check for damage"),
    ("scrub", "check for damage"),
    ("fsck", "check for damage"),
    ("military-grade", "(forbidden claim)"),
    ("bank-level", "(forbidden claim)"),
    ("unbreakable", "(forbidden claim)"),
    ("hacker-proof", "(forbidden claim)"),
    ("100% secure", "(forbidden claim)"),
    ("your data is safe", "(forbidden claim)"),
    ("padlock", "(anti-goal iconography, §1.2)"),
    ("keyhole", "(anti-goal iconography, §1.2)"),
];

/// Extracted string literals that are not prose — an exact match, not a
/// substring, so allowing `"entry"` (the `ListRow` discriminant tag) does
/// not also silence a real future violation like `"Delete this entry?"`.
/// Every entry says what it actually is and why Design §7 does not apply.
const ALLOWLIST_EXACT: &[&str] = &[
    "operation-progress",   // a Tauri event name, not shown to anyone
    "entry",                // `ListRow`'s `kind: "entry"` discriminant, not text
    "object",               // `typeof value === "object"`, not text
    "entry-row",            // CSS class name
    "entry-row unreadable", // CSS class name
    ".entry-row",           // CSS selector
    "extract_entry",        // Tauri command name
    "delete_entry",         // Tauri command name
    "replace_entry",        // Tauri command name
];

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Strips `//` and `/* */` comments first — both languages' prose is full
/// of contractions ("doesn't", "vault's"), and a bare apostrophe read as a
/// single-quote string delimiter turns half a file's comments into one
/// "string literal". Neither language uses single-quoted strings for
/// anything this test needs to see (Rust's are char literals; this
/// codebase's TypeScript uses double quotes), so single quotes are not
/// treated as a literal delimiter at all — sidestepping the ambiguity
/// rather than resolving it.
fn strip_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_string: Option<char> = None;
    while let Some(c) = chars.next() {
        if let Some(quote) = in_string {
            out.push(c);
            if c == '\\' {
                if let Some(escaped) = chars.next() {
                    out.push(escaped);
                }
            } else if c == quote {
                in_string = None;
            }
            continue;
        }
        match c {
            '"' | '`' => {
                in_string = Some(c);
                out.push(c);
            }
            '/' if chars.peek() == Some(&'/') => {
                for next in chars.by_ref() {
                    if next == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut prev = ' ';
                for next in chars.by_ref() {
                    if prev == '*' && next == '/' {
                        break;
                    }
                    prev = next;
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// Every `"..."` or `` `...` `` literal's contents, from source text with
/// comments already stripped.
fn string_literals(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let stripped = strip_comments(source);
    let mut chars = stripped.chars().peekable();
    while let Some(c) = chars.next() {
        let quote = match c {
            '"' | '`' => c,
            _ => continue,
        };
        let mut literal = String::new();
        let mut escaped = false;
        while let Some(next) = chars.next() {
            if escaped {
                literal.push(next);
                escaped = false;
                continue;
            }
            if next == '\\' {
                escaped = true;
                continue;
            }
            if next == quote {
                break;
            }
            // A template literal's `${expr}` is code, not prose — `entry`
            // in `` `Delete ${entry.name}?` `` is the variable Design §7
            // explicitly permits internally; what a person reads is
            // whatever it evaluates to, never the word "entry". Skipped
            // rather than checked, brace-depth aware for a nested call.
            if quote == '`' && next == '$' && chars.peek() == Some(&'{') {
                chars.next();
                let mut depth = 1;
                for inner in chars.by_ref() {
                    match inner {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                literal.push(' ');
                continue;
            }
            literal.push(next);
        }
        out.push(literal);
    }
    out
}

fn check_dir(dir: &Path, extension: &str, violations: &mut Vec<String>) {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().and_then(|n| n.to_str()) != Some("node_modules") {
                    stack.push(path);
                }
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some(extension) {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            for literal in string_literals(&source) {
                if ALLOWLIST_EXACT.contains(&literal.trim()) {
                    continue;
                }
                let lower = literal.to_lowercase();
                for (banned, use_instead) in DENYLIST {
                    if contains_word(&lower, banned) {
                        violations.push(format!(
                            "{}: {literal:?} contains {banned:?} (use {use_instead:?})",
                            path.display()
                        ));
                    }
                }
            }
        }
    }
}

/// Whole-word / whole-phrase containment, so `"key"` inside `"keyboard"`
/// (not itself banned, but illustrates the risk) would not false-positive
/// — and, more to the point, so `"container"` inside a longer unrelated
/// identifier does not either.
fn contains_word(haystack: &str, needle: &str) -> bool {
    let mut start = 0;
    while let Some(pos) = haystack[start..].find(needle) {
        let abs = start + pos;
        let before_ok = haystack[..abs]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let after_ok = haystack[abs + needle.len()..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric());
        if before_ok && after_ok {
            return true;
        }
        start = abs + needle.len();
    }
    false
}

/// T6.31 — the vocabulary audit is clean in both applications.
#[test]
fn t6_31_the_vocabulary_audit_is_clean_in_both_applications() {
    let mut violations = Vec::new();

    check_dir(&manifest_dir().join("ui/src"), "ts", &mut violations);

    let cli_src = manifest_dir().parent().unwrap().join("veil-cli/src");
    check_dir(&cli_src, "rs", &mut violations);

    assert!(
        violations.is_empty(),
        "the vocabulary audit found {} violation(s):\n  {}",
        violations.len(),
        violations.join("\n  ")
    );
}
