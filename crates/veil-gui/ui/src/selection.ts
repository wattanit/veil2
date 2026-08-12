// Multi-select transitions (P8.3, Design §3.2): a plain click selects one
// row and clears the rest; shift-click extends a contiguous range from the
// anchor (the last plain- or Cmd-clicked row) to the clicked row, in the
// list's current visual order; Cmd-click toggles one row without disturbing
// the rest. Extracted as a pure function, rather than left inline in
// main.ts's DOM handling, so it is testable the way extension.ts and
// sort.ts are (T8.7).

export type ClickKind = "plain" | "shift" | "cmd";

export interface SelectionState {
  selectedIds: Set<number>;
  // The shift-range anchor — the row a plain or Cmd-click last landed on.
  // `null` only before anything has ever been clicked this session.
  lastClickedId: number | null;
}

export function nextSelection(
  state: SelectionState,
  visualOrder: number[],
  clickedId: number,
  kind: ClickKind,
): SelectionState {
  if (kind === "plain") {
    return { selectedIds: new Set([clickedId]), lastClickedId: clickedId };
  }

  if (kind === "cmd") {
    const selectedIds = new Set(state.selectedIds);
    if (selectedIds.has(clickedId)) {
      selectedIds.delete(clickedId);
    } else {
      selectedIds.add(clickedId);
    }
    return { selectedIds, lastClickedId: clickedId };
  }

  // Shift-click with no anchor yet (nothing clicked this session) has
  // nothing to extend from, so it behaves like a plain click.
  const anchorIndex = state.lastClickedId === null ? -1 : visualOrder.indexOf(state.lastClickedId);
  const clickedIndex = visualOrder.indexOf(clickedId);
  if (anchorIndex === -1 || clickedIndex === -1) {
    return { selectedIds: new Set([clickedId]), lastClickedId: clickedId };
  }

  const [start, end] =
    anchorIndex <= clickedIndex ? [anchorIndex, clickedIndex] : [clickedIndex, anchorIndex];
  const selectedIds = new Set(visualOrder.slice(start, end + 1));
  // The anchor itself does not move — a second shift-click re-extends from
  // the same anchor, not from wherever the previous shift-click landed.
  return { selectedIds, lastClickedId: state.lastClickedId };
}
