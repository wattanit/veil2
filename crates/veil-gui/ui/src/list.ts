// The virtualised entry list (P5.4): only rows near the viewport ever exist
// in the DOM. Extracted from Phase 5's main.ts unchanged in behaviour, now
// parameterised over whatever filtered/grouped view of the entries main.ts
// hands it (P6.5), rather than always the full list.
import type { EntryInfo } from "./api";

const ROW_HEIGHT = 28;
const OVERSCAN = 4;

// A group header renders differently from an entry row (P6.5.b) and has no
// entry of its own to carry. `key` identifies the group for collapse
// tracking (P8.1.c) independently of `label`, which is the resolved,
// never-empty text main.ts has already chosen for the group's grouping mode
// (e.g. "(no extension)") — this module renders it as-is rather than
// re-deriving a fallback for an empty group.
export type ListRow =
  | { kind: "entry"; entry: EntryInfo }
  | { kind: "group"; key: string; label: string; count: number; collapsed: boolean };

export class EntryList {
  private rows: ListRow[] = [];
  private readonly pool = new Map<number, HTMLDivElement>();
  private readonly onActivate: (entry: EntryInfo) => void;
  // P8.3: held here, not just applied as a one-off DOM mutation from the
  // click handler that selected a row — `renderRow` reads it on every call,
  // including the ones a scroll or a resize triggers for a pooled element
  // that already existed. Before this, a row's "selected" class only ever
  // came from main.ts touching that element directly at click time; the
  // very next `renderVisible()` (any scroll, at all) overwrote `className`
  // wholesale and silently dropped it. Multi-select made that worth fixing
  // rather than working around, since it is far more visible with several
  // rows selected than it ever was with one.
  private selected = new Set<number>();

  constructor(
    private readonly scrollEl: HTMLElement,
    private readonly spacerEl: HTMLElement,
    handlers: {
      onActivate: (entry: EntryInfo) => void;
    },
  ) {
    this.onActivate = handlers.onActivate;
    this.scrollEl.addEventListener("scroll", () => this.renderVisible());
    window.addEventListener("resize", () => this.renderVisible());
    this.spacerEl.addEventListener("dblclick", (event) => {
      const entry = this.entryFromEvent(event);
      if (entry) {
        this.onActivate(entry);
      }
    });
  }

  setRows(rows: ListRow[]): void {
    this.rows = rows;
    this.renderVisible();
  }

  // The ids of every entry row in the current rows, in the current visual
  // order — a collapsed group's entries are absent from `rows` already, so
  // they are absent here too (P8.3's shift-range only ever spans what is
  // actually visible).
  entryIds(): number[] {
    const ids: number[] = [];
    for (const row of this.rows) {
      if (row.kind === "entry") {
        ids.push(row.entry.id);
      }
    }
    return ids;
  }

  setSelection(ids: Set<number>): void {
    this.selected = ids;
    this.renderVisible();
  }

  private entryById(id: string): EntryInfo | undefined {
    for (const row of this.rows) {
      if (row.kind === "entry" && String(row.entry.id) === id) {
        return row.entry;
      }
    }
    return undefined;
  }

  private entryFromEvent(event: Event): EntryInfo | undefined {
    const row = (event.target as HTMLElement).closest<HTMLElement>(".entry-row");
    const id = row?.dataset.id;
    return id === undefined ? undefined : this.entryById(id);
  }

  renderVisible(): void {
    this.spacerEl.style.height = `${this.rows.length * ROW_HEIGHT}px`;

    const viewportHeight = this.scrollEl.clientHeight;
    const scrollTop = this.scrollEl.scrollTop;
    const first = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN);
    const last = Math.min(
      this.rows.length - 1,
      Math.ceil((scrollTop + viewportHeight) / ROW_HEIGHT) + OVERSCAN,
    );

    for (const [index, el] of this.pool) {
      if (index < first || index > last) {
        el.remove();
        this.pool.delete(index);
      }
    }

    for (let index = first; index <= last; index += 1) {
      const row = this.rows[index];
      if (!row) {
        continue;
      }
      let el = this.pool.get(index);
      if (!el) {
        el = document.createElement("div");
        this.spacerEl.appendChild(el);
        this.pool.set(index, el);
      }
      this.renderRow(el, row, index);
    }
  }

  private renderRow(el: HTMLDivElement, row: ListRow, index: number): void {
    el.style.top = `${index * ROW_HEIGHT}px`;
    if (row.kind === "group") {
      el.className = row.collapsed ? "group-header collapsed" : "group-header";
      delete el.dataset.id;
      el.dataset.key = row.key;
      el.innerHTML =
        '<span class="group-caret" aria-hidden="true"></span>' +
        `<span class="group-label">${escapeHtml(row.label)}</span>` +
        `<span class="group-count">${row.count.toLocaleString()}</span>`;
      return;
    }
    const entry = row.entry;
    el.className = entry.unreadable ? "entry-row unreadable" : "entry-row";
    if (this.selected.has(entry.id)) {
      el.classList.add("selected");
    }
    el.dataset.id = String(entry.id);
    el.innerHTML =
      `<span class="col-name">${escapeHtml(entry.name)}</span>` +
      `<span class="col-folder">${escapeHtml(entry.folder)}</span>` +
      `<span class="col-size">${formatSize(entry.size)}</span>` +
      `<span class="col-added">${formatDate(entry.addedAt)}</span>`;
  }

}

function formatSize(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const precision = unit === 0 ? 0 : 1;
  return `${value.toFixed(precision)} ${units[unit]}`;
}

// Shared by the Added column here and the details panel's Added/Modified
// fields (P8.5) — one date rendering, not a second copy of it for the
// panel to drift from.
export function formatDate(epochSeconds: number): string {
  return new Date(epochSeconds * 1000).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

function escapeHtml(value: string): string {
  const div = document.createElement("div");
  div.textContent = value;
  return div.innerHTML;
}
