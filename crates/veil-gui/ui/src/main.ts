// Phase 5 foundation shell (Design §3.1–§3.3, §2.3). No unlock screen, no
// vault creation, no identity bar — those are Phase 6. What this proves is
// that the entry list renders correctly and densely, that complex-script
// names render correctly in both themes (P5.5), and that the window behaves
// as a drop target (P5.6).
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";

interface EntryInfo {
  id: number;
  name: string;
  folder: string;
  size: number;
  addedAt: number;
}

const ROW_HEIGHT = 28;
const OVERSCAN = 4;

function requireElement<T extends HTMLElement>(selector: string): T {
  const el = document.querySelector<T>(selector);
  if (!el) {
    throw new Error(`index.html is missing ${selector}`);
  }
  return el;
}

const statusEl = requireElement<HTMLDivElement>("#status");
const dropEl = requireElement<HTMLDivElement>("#drop-affordance");
const scrollEl = requireElement<HTMLDivElement>("#list-scroll");
const spacerEl = requireElement<HTMLDivElement>("#list-spacer");

let entries: EntryInfo[] = [];
const rowPool = new Map<number, HTMLDivElement>();

function setStatus(message: string): void {
  statusEl.textContent = message;
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

function formatAdded(epochSeconds: number): string {
  return new Date(epochSeconds * 1000).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

function renderRow(el: HTMLDivElement, entry: EntryInfo, index: number): void {
  el.className = "entry-row";
  el.style.top = `${index * ROW_HEIGHT}px`;
  el.dataset.id = String(entry.id);
  el.innerHTML =
    `<span class="col-name">${escapeHtml(entry.name)}</span>` +
    `<span class="col-folder">${escapeHtml(entry.folder)}</span>` +
    `<span class="col-size">${formatSize(entry.size)}</span>` +
    `<span class="col-added">${formatAdded(entry.addedAt)}</span>`;
}

function escapeHtml(value: string): string {
  const div = document.createElement("div");
  div.textContent = value;
  return div.innerHTML;
}

// A windowed view: only rows near the viewport ever exist in the DOM (P5.4.d,
// Design §2.3), regardless of how many thousand entries a vault holds.
function renderVisible(): void {
  spacerEl.style.height = `${entries.length * ROW_HEIGHT}px`;

  const viewportHeight = scrollEl.clientHeight;
  const scrollTop = scrollEl.scrollTop;
  const first = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN);
  const last = Math.min(
    entries.length - 1,
    Math.ceil((scrollTop + viewportHeight) / ROW_HEIGHT) + OVERSCAN,
  );

  for (const [index, el] of rowPool) {
    if (index < first || index > last) {
      el.remove();
      rowPool.delete(index);
    }
  }

  for (let index = first; index <= last; index += 1) {
    const entry = entries[index];
    if (!entry) {
      continue;
    }
    let el = rowPool.get(index);
    if (!el) {
      el = document.createElement("div");
      spacerEl.appendChild(el);
      rowPool.set(index, el);
    }
    renderRow(el, entry, index);
  }
}

async function loadEntries(): Promise<void> {
  entries = await invoke<EntryInfo[]>("list_entries");
  renderVisible();
}

async function openFixture(): Promise<void> {
  try {
    const summary = await invoke<{ entryCount: number }>("open_fixture_vault");
    setStatus(`${summary.entryCount} files (fixture vault)`);
    await loadEntries();
  } catch (error) {
    // The fixture command is debug-only (Phase5-ToDo.md, P5.1/scope note) —
    // in a release build this fails, and there is deliberately no unlock
    // screen yet to fall back to. That is Phase 6's job.
    setStatus("No vault open.");
    console.info("open_fixture_vault unavailable:", error);
  }
}

function setupDropTarget(): void {
  void getCurrentWebview().onDragDropEvent((event) => {
    const payload = event.payload;
    if (payload.type === "enter" || payload.type === "over") {
      const count = "paths" in payload ? payload.paths.length : 0;
      if (count > 0) {
        dropEl.textContent = `Add ${count} file${count === 1 ? "" : "s"}`;
        dropEl.hidden = false;
      }
      return;
    }
    if (payload.type === "leave") {
      dropEl.hidden = true;
      return;
    }
    if (payload.type === "drop") {
      dropEl.hidden = true;
      void handleDrop(payload.paths);
    }
  });
}

async function handleDrop(paths: string[]): Promise<void> {
  if (paths.length === 0) {
    return;
  }
  setStatus(`Adding ${paths.length} file${paths.length === 1 ? "" : "s"}…`);
  try {
    await invoke("add_files", { paths });
    await loadEntries();
    setStatus(`${entries.length} files`);
  } catch (error) {
    setStatus(`Could not add files: ${String(error)}`);
  }
}

// Double-clicking a row extracts it (P5.6.d, Design §3.3, FR-17). Delegated
// on the spacer rather than attached per row, since rows are pooled and
// reused as the list scrolls (renderVisible).
function setupExtraction(): void {
  spacerEl.addEventListener("dblclick", (event) => {
    const row = (event.target as HTMLElement).closest<HTMLElement>(".entry-row");
    const id = row?.dataset.id;
    if (id === undefined) {
      return;
    }
    const entry = entries.find((candidate) => String(candidate.id) === id);
    if (entry) {
      void handleExtract(entry);
    }
  });
}

async function handleExtract(entry: EntryInfo): Promise<void> {
  const destination = await invoke<string | null>("choose_save_path", {
    suggestedName: entry.name,
  });
  if (!destination) {
    return;
  }
  setStatus(`Extracting ${entry.name}…`);
  try {
    await invoke("extract_entry", { id: entry.id, destination });
    setStatus(`Saved ${entry.name}`);
  } catch (error) {
    setStatus(`Could not extract ${entry.name}: ${String(error)}`);
  }
}

scrollEl.addEventListener("scroll", renderVisible);
window.addEventListener("resize", renderVisible);
setupDropTarget();
setupExtraction();
void openFixture();
