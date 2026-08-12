// File extension derivation (FR-29), extracted as its own module because it
// is duplicated deliberately: the CLI (`crates/veil-cli/src/extension.rs`)
// implements the same rule independently rather than sharing it through
// `veil-core` (Tech Spec §5.1). `crates/veil-cli/tests/extension_parity.rs`
// is what keeps the two from drifting apart.
//
// Not wired into the grouping control yet — that lands in Phase 8. This
// module exists on its own so Phase 7's parity test has something to check.

/// The substring of `name` after its last `.`, lowercased. `null` if `name`
/// has no `.`, if that `.` is the first character (a dotfile has no
/// extension), or if nothing follows it (a trailing dot).
export function extensionOf(name: string): string | null {
  const dot = name.lastIndexOf(".");
  if (dot <= 0 || dot === name.length - 1) {
    return null;
  }
  return name.slice(dot + 1).toLowerCase();
}
