// Column sort (P8.2, Design §3.2): clicking a header sorts ascending, a
// second click on the same header reverses to descending. Extracted as pure
// functions, rather than left inline in main.ts's DOM handling, so this is
// testable the way extension.ts is (T8.4) instead of only by driving the
// list live.
import type { EntryInfo } from "./api";

export type SortColumn = "name" | "folder" | "size" | "added";
export type SortDirection = "asc" | "desc";

// Name and folder read as text a person typed, so they sort the way a
// person expects text to sort — case-insensitively — not by code point.
const ascendingComparators: Record<SortColumn, (a: EntryInfo, b: EntryInfo) => number> = {
  name: (a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }),
  folder: (a, b) => a.folder.localeCompare(b.folder, undefined, { sensitivity: "base" }),
  size: (a, b) => a.size - b.size,
  added: (a, b) => a.addedAt - b.addedAt,
};

export function compareEntries(
  a: EntryInfo,
  b: EntryInfo,
  column: SortColumn,
  direction: SortDirection,
): number {
  const ascending = ascendingComparators[column](a, b);
  return direction === "asc" ? ascending : -ascending;
}

// A new array — `Array.prototype.sort` is stable (ECMA-262 since ES2019),
// so entries that tie on the chosen column keep their relative order rather
// than shuffling on every click.
export function sortEntries(
  entries: EntryInfo[],
  column: SortColumn,
  direction: SortDirection,
): EntryInfo[] {
  return [...entries].sort((a, b) => compareEntries(a, b, column, direction));
}
