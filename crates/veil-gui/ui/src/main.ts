// Phase 6: the product. Phase 5 built the shell and the list; this file
// adds the screens Design's Key Moments (§5, §8.1-8.8) describe: first run,
// unlock, creation, the identity bar, search and grouping, add/extract/
// delete/replace, locking, checking for damage, and changing the password.
import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import * as api from "./api";
import type { EntryInfo } from "./api";
import { extensionOf } from "./extension";
import { EntryList, type ListRow } from "./list";
import { isPreviewable } from "./previewable";
import { nextSelection, type ClickKind } from "./selection";
import { sortEntries, type SortColumn, type SortDirection } from "./sort";

const MIN_PASSWORD_LENGTH = 12; // C-4, mirrored client-side; veil-core is the authority.

// ------------------------------------------------------------------ DOM ---

function el<T extends HTMLElement>(id: string): T {
  const found = document.getElementById(id);
  if (!found) {
    throw new Error(`index.html is missing #${id}`);
  }
  return found as T;
}

const screens = {
  firstRun: el<HTMLElement>("screen-first-run"),
  unlock: el<HTMLElement>("screen-unlock"),
  create: el<HTMLElement>("screen-create"),
  locked: el<HTMLElement>("screen-locked"),
  vault: el<HTMLElement>("screen-vault"),
};

function showScreen(name: keyof typeof screens): void {
  for (const [key, section] of Object.entries(screens)) {
    section.hidden = key !== name;
  }
}

// -------------------------------------------------------------- naming ---

// A vault's name is its directory's filename — neither the header nor the
// index records one (Phase6-ToDo.md's own note). Strips a trailing
// `.veil` the same way a person reading the path would.
function vaultNameFromPath(path: string): string {
  const base = path.replace(/[/\\]+$/, "").split(/[/\\]/).pop() ?? path;
  return base.replace(/\.veil$/i, "");
}

// ------------------------------------------------------------- session ---

// What screen-vault, once open, needs to remember between actions.
let currentPath = "";
let currentSummary: api.VaultSummary | null = null;
let allEntries: EntryInfo[] = [];

// P8.3: click selects one row and clears the rest, shift-click extends a
// contiguous range, Cmd-click toggles one row (Design §3.2) — replaces the
// single `selectedId` of Phase 6. `lastClickedId` is the shift-range
// anchor, not just "the most recently selected row": a Cmd-click that
// removes a row from the selection still becomes the anchor for the next
// shift-click.
let selectedIds = new Set<number>();
let lastClickedId: number | null = null;
let searchTerm = "";

// P8.1: one choice among none, by folder, and by extension (Design §3.2,
// FR-8, FR-29) — never both dimensions at once.
type Grouping = "none" | "folder" | "extension";
let grouping: Grouping = "none";

// Which groups are collapsed, by group key. Cleared whenever the grouping
// choice changes or the vault is locked and reopened (Design §3.2) — never
// otherwise, so a collapse survives an unrelated search or refresh.
let collapsedGroups = new Set<string>();

// P8.2: no sort until a header is clicked once — `null` means "whatever
// order list_entries returned", the same order the list showed before this
// phase. There is no separate sort control (Design §3.2); a header click is
// the only way to reach either field.
let sortColumn: SortColumn | null = null;
let sortDirection: SortDirection = "asc";

// ------------------------------------------------------- first run (§8.1) -

el<HTMLButtonElement>("first-run-create").addEventListener("click", () => {
  void startCreate();
});
el<HTMLButtonElement>("first-run-open").addEventListener("click", () => {
  void startOpen();
});

// Read from the running build rather than hardcoded, so it can't drift
// from tauri.conf.json's own version field.
void getVersion().then((version) => {
  el<HTMLElement>("first-run-version").textContent = `v${version}`;
});

async function startOpen(): Promise<void> {
  const path = await api.chooseVaultPath("open");
  if (!path) {
    return;
  }
  currentPath = path;
  el<HTMLElement>("unlock-name").textContent = vaultNameFromPath(path);
  el<HTMLElement>("unlock-location").textContent = path;
  setUnlockOutcome(null);
  el<HTMLInputElement>("unlock-password").value = "";
  showScreen("unlock");
}

async function startCreate(): Promise<void> {
  const path = await api.chooseVaultPath("create");
  if (!path) {
    return;
  }
  currentPath = path;
  el<HTMLElement>("create-name").textContent = vaultNameFromPath(path);
  el<HTMLElement>("create-location").textContent = path;
  const form = el<HTMLFormElement>("create-form");
  form.reset();
  el<HTMLButtonElement>("create-button").disabled = true;
  showScreen("create");
}

// --------------------------------------------------------- unlock (§5) ---

function setUnlockOutcome(html: string | null): void {
  const outcome = el<HTMLElement>("unlock-outcome");
  if (html === null) {
    outcome.hidden = true;
    outcome.innerHTML = "";
    return;
  }
  outcome.hidden = false;
  outcome.innerHTML = html;
}

el<HTMLFormElement>("unlock-form").addEventListener("submit", (event) => {
  event.preventDefault();
  void unlock();
});

async function unlock(): Promise<void> {
  const password = el<HTMLInputElement>("unlock-password").value;
  const button = el<HTMLButtonElement>("unlock-button");
  const field = el<HTMLInputElement>("unlock-password");
  setUnlockOutcome(null);
  button.classList.add("working");
  button.disabled = true;
  field.disabled = true;
  try {
    const summary = await api.openVault(currentPath, password);
    await enterVault(summary);
  } catch (raw) {
    const error = api.describeError(raw);
    setUnlockOutcome(unlockOutcomeHtml(error));
  } finally {
    button.classList.remove("working");
    button.disabled = false;
    field.disabled = false;
  }
}

// FR-2: wrong password and a damaged vault are different messages, never
// the same "something went wrong". FR-5/FR-6: format mismatches name the
// versions involved.
function unlockOutcomeHtml(error: api.ErrorInfo): string {
  switch (error.kind) {
    case "WrongPassword":
      return "<p>That password didn&rsquo;t work. Try again.</p>";
    case "NotAVault":
    case "Corrupt":
      return (
        "<p><strong>This vault can&rsquo;t be read.</strong> It may be incomplete or damaged.</p>" +
        `<p>${escapeHtml(error.message)}</p>` +
        "<p>If you have a backup, work from that copy instead of retrying here.</p>"
      );
    case "FormatTooNew":
      return `<p>${escapeHtml(error.message)}</p>`;
    case "FormatSuperseded":
      return (
        `<p>${escapeHtml(error.message)}</p>` +
        "<p>A future release may offer to convert it.</p>"
      );
    case "VaultInUse":
      return `<p>${escapeHtml(error.message)}</p><p>Close it elsewhere, then try again.</p>`;
    default:
      return `<p>${escapeHtml(error.message)}</p>`;
  }
}

// ---------------------------------------------------- creating (§8.2) ---

const createPassword = el<HTMLInputElement>("create-password");
const createConfirm = el<HTMLInputElement>("create-password-confirm");
const createAcknowledge = el<HTMLInputElement>("create-acknowledge");
const createButton = el<HTMLButtonElement>("create-button");
const createHint = el<HTMLElement>("create-password-hint");

function updateCreateValidity(): void {
  const password = createPassword.value;
  let hint = "";
  if (password.length > 0 && password.length < MIN_PASSWORD_LENGTH) {
    hint = `A vault password must be at least ${MIN_PASSWORD_LENGTH} characters`;
  } else if (createConfirm.value.length > 0 && createConfirm.value !== password) {
    hint = "Those two passwords don't match";
  }
  createHint.textContent = hint;
  createHint.hidden = hint === "";

  createButton.disabled =
    password.length < MIN_PASSWORD_LENGTH ||
    password !== createConfirm.value ||
    !createAcknowledge.checked;
}

for (const input of [createPassword, createConfirm, createAcknowledge]) {
  input.addEventListener("input", updateCreateValidity);
}

el<HTMLFormElement>("create-form").addEventListener("submit", (event) => {
  event.preventDefault();
  void create();
});

async function create(): Promise<void> {
  try {
    const summary = await api.createVault(currentPath, createPassword.value);
    await enterVault(summary);
  } catch (raw) {
    const error = api.describeError(raw);
    createHint.textContent = error.message;
    createHint.hidden = false;
  }
}

// --------------------------------------------------------- locking (§8.5) -

el<HTMLButtonElement>("identity-lock").addEventListener("click", () => {
  void lock();
});
el<HTMLButtonElement>("locked-unlock-again").addEventListener("click", () => {
  el<HTMLElement>("unlock-name").textContent = vaultNameFromPath(currentPath);
  el<HTMLElement>("unlock-location").textContent = currentPath;
  setUnlockOutcome(null);
  el<HTMLInputElement>("unlock-password").value = "";
  showScreen("unlock");
});

async function lock(): Promise<void> {
  await api.closeVault();
  el<HTMLElement>("locked-name").textContent = vaultNameFromPath(currentPath);
  currentSummary = null;
  allEntries = [];
  clearSelection();
  // The context menu is a sibling of the screens, not inside #screen-vault
  // — left open, it would float over the locked screen bound to entries
  // that no longer exist.
  closeContextMenu();
  showScreen("locked");
}

// Quitting the app locks the vault (Design §8.5, FR-3) — there is no
// "leave it open" setting, so no handler here offers one. The Rust side
// holds no vault-specific cleanup that outlives the process; state simply
// stops existing when the process does.

// -------------------------------------------------------- vault screen ---

const list = new EntryList(el("list-scroll"), el("list-spacer"), {
  onActivate: (entry) => {
    void extract(entry);
  },
});

async function enterVault(summary: api.VaultSummary): Promise<void> {
  currentSummary = summary;
  clearSelection();
  searchTerm = "";
  el<HTMLInputElement>("search-input").value = "";
  collapsedGroups.clear();
  el<HTMLElement>("identity-name").textContent = vaultNameFromPath(currentPath);
  const readonlyNote = el<HTMLElement>("identity-readonly-note");
  readonlyNote.hidden = summary.access !== "readOnly";
  setControlsDisabled(summary.access === "readOnly");
  showScreen("vault");
  await refreshList();
}

function setControlsDisabled(disabled: boolean): void {
  for (const id of ["add-files-button", "identity-change-password"]) {
    el<HTMLButtonElement>(id).disabled = disabled;
    el<HTMLButtonElement>(id).title = disabled
      ? "Read-only — this vault can't be changed from here."
      : "";
  }
}

async function refreshList(): Promise<void> {
  allEntries = await api.listEntries();
  renderStatistics();
  renderList();
}

function renderStatistics(): void {
  const bytes = allEntries.reduce((sum, e) => sum + e.size, 0);
  el<HTMLElement>("statistics-line").textContent =
    `${allEntries.length.toLocaleString()} files · ${humanSize(bytes)} stored`;
}

function humanSize(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`;
}

function visibleEntries(): EntryInfo[] {
  const term = searchTerm.trim().toLowerCase();
  if (term === "") {
    return allEntries;
  }
  return allEntries.filter(
    (e) => e.name.toLowerCase().includes(term) || e.folder.toLowerCase().includes(term),
  );
}

// P6.5.b, P8.1: a flat grouping view, not a tree — one level, no
// create/rename/drag, only collapse/expand (Design §3.2).
function groupKey(entry: EntryInfo): string {
  return grouping === "extension" ? extensionOf(entry.name) ?? "" : entry.folder;
}

// Never empty — an empty key gets the grouping mode's own reserved label
// (Design §3.2), the same word the CLI's peer output uses for the same
// case (`output::label` in `veil-cli`) where a shared word exists.
function groupLabel(key: string): string {
  if (key !== "") {
    return key;
  }
  return grouping === "extension" ? "(no extension)" : "(root)";
}

// P8.2.c: applied before grouping, not after — a `Map`'s buckets fill in
// the order entries are pushed into them, so sorting the flat list first
// leaves each group's own bucket in that same sorted order, with no second
// sort pass needed per group.
function applySort(entries: EntryInfo[]): EntryInfo[] {
  return sortColumn === null ? entries : sortEntries(entries, sortColumn, sortDirection);
}

function renderList(): void {
  const entries = applySort(visibleEntries());
  if (grouping === "none") {
    list.setRows(entries.map((entry): ListRow => ({ kind: "entry", entry })));
    return;
  }
  const byGroup = new Map<string, EntryInfo[]>();
  for (const entry of entries) {
    const key = groupKey(entry);
    const bucket = byGroup.get(key) ?? [];
    bucket.push(entry);
    byGroup.set(key, bucket);
  }
  const rows: ListRow[] = [];
  for (const [key, entriesInGroup] of [...byGroup.entries()].sort(([a], [b]) =>
    a.localeCompare(b),
  )) {
    const collapsed = collapsedGroups.has(key);
    rows.push({
      kind: "group",
      key,
      label: groupLabel(key),
      count: entriesInGroup.length,
      collapsed,
    });
    if (!collapsed) {
      for (const entry of entriesInGroup) {
        rows.push({ kind: "entry", entry });
      }
    }
  }
  list.setRows(rows);
}

el<HTMLInputElement>("search-input").addEventListener("input", (event) => {
  searchTerm = (event.target as HTMLInputElement).value;
  renderList();
});
el<HTMLSelectElement>("group-select").addEventListener("change", (event) => {
  grouping = (event.target as HTMLSelectElement).value as Grouping;
  // Design §3.2: changing the grouping choice returns every group to
  // expanded — the previous mode's collapsed keys have no meaning under
  // the new one anyway (a folder path is not an extension).
  collapsedGroups.clear();
  renderList();
});

// P8.2.a, P8.2.b: a click on a header sorts ascending by it; a second click
// on the same header reverses to descending; a click on a different header
// starts that column over at ascending.
function updateSortArrows(): void {
  for (const header of el<HTMLElement>("list-header").querySelectorAll<HTMLElement>(
    "[data-column]",
  )) {
    const arrow = header.querySelector<HTMLElement>(".sort-arrow");
    if (!arrow) {
      continue;
    }
    arrow.textContent =
      header.dataset.column === sortColumn ? (sortDirection === "asc" ? " ▲" : " ▼") : "";
  }
}

el<HTMLElement>("list-header").addEventListener("click", (event) => {
  const header = (event.target as HTMLElement).closest<HTMLElement>("[data-column]");
  const column = header?.dataset.column as SortColumn | undefined;
  if (!column) {
    return;
  }
  if (sortColumn === column) {
    sortDirection = sortDirection === "asc" ? "desc" : "asc";
  } else {
    sortColumn = column;
    sortDirection = "asc";
  }
  updateSortArrows();
  renderList();
});

// --------------------------------------------------------- status/error --

function setStatus(message: string): void {
  el<HTMLElement>("status").textContent = message;
}

// P6.10: three parts — what happened, the current state, what can be done
// — rendered at the action's own location (here, the status line beneath
// the controls bar) rather than as a system notification.
function describeForStatus(action: string, error: api.ErrorInfo): string {
  const advice: Record<string, string> = {
    VaultInUse: "Close it elsewhere, then try again.",
    ChangedOnDisk: "Reload the vault to see the change.",
    StorageUnavailable: "Reconnect the volume and try again.",
    LimitExceeded: "Remove something first, or use a smaller file.",
    ReadOnly: "This vault can't be changed from here.",
    Cancelled: "",
  };
  const suffix = advice[error.kind] ? ` ${advice[error.kind]}` : "";
  return `${action}: ${error.message}${suffix}`;
}

// -------------------------------------------------------- operation bar --

function showOperation(label: string): void {
  el<HTMLElement>("operation-bar").hidden = false;
  el<HTMLElement>("operation-label").textContent = label;
  el<HTMLElement>("operation-progress-fill").style.width = "0%";
}

function hideOperation(): void {
  el<HTMLElement>("operation-bar").hidden = true;
}

el<HTMLButtonElement>("operation-cancel").addEventListener("click", () => {
  void api.cancelOperation();
});

interface ProgressPayload {
  done: number;
  total: number | null;
}

void getCurrentWebview().listen<ProgressPayload>("operation-progress", (event) => {
  const { done, total } = event.payload;
  if (total) {
    el<HTMLElement>("operation-progress-fill").style.width = `${Math.min(100, (done / total) * 100)}%`;
  }
});

// -------------------------------------------------------------- extract --

async function extract(entry: EntryInfo): Promise<void> {
  const destination = await api.chooseSavePath(entry.name);
  if (!destination) {
    return;
  }
  // FR-19's overwrite confirmation is the native save dialog's own — macOS
  // asks before returning a path that already exists, so there is nothing
  // for this layer to add on top of the dialog it already used.
  showOperation(`Saving a copy of ${entry.name}…`);
  try {
    await api.extractEntry(entry.id, destination);
    hideOperation();
    // FR-27: stated every time, plainly, as completion-state text.
    setStatus(`Saved to ${folderOf(destination)}. This copy is not protected.`);
  } catch (raw) {
    hideOperation();
    setStatus(describeForStatus(`Couldn't save a copy of ${entry.name}`, api.describeError(raw)));
  }
}

function folderOf(path: string): string {
  const parts = path.split(/[/\\]/);
  parts.pop();
  return parts.pop() ?? path;
}

// Save as… (Design §3.5) for a whole selection — one native save dialog per
// file, exactly `extract()` as double-click already invokes it, run in
// sequence. Design §3.5 describes this as "the same destination-choosing
// extraction the row's double-click... already perform[s]", which is a
// single-file operation; there is no destination-folder picker anywhere in
// this codebase to extend it into one native "choose a folder, write every
// file there" flow, and building one would also need its own overwrite
// check (Design §4.1/FR-19), since a folder-picker carries none of the
// per-file confirmation a save dialog gives for free. N sequential dialogs
// is the honest, zero-new-surface reading for a first cut; a batch
// destination is left open (Phase8-ToDo.md's own note) rather than
// invented here.
async function extractSelection(entries: EntryInfo[]): Promise<void> {
  for (const entry of entries) {
    await extract(entry);
  }
}

// ------------------------------------------------------------------ add --

el<HTMLButtonElement>("add-files-button").addEventListener("click", () => {
  void addViaDialog();
});

async function addViaDialog(): Promise<void> {
  const paths = await api.chooseSourcePaths(true);
  if (paths.length > 0) {
    await runAdd(paths);
  }
}

// A dropped or chosen path whose folder and name already match an entry is
// not added or silently replaced — it is held by the backend as a
// `Collision` (P6.0's `add_files`) and confirmed here before anything is
// replaced (Design §8.7, §4.1's irreversible-action rule). Matched by
// identity, never by where in the window a drop happened to land: an
// earlier version tried to detect "dropped onto row X" from the drag's
// on-screen position, which real testing showed was unreliable in this
// Tauri version — the position it reports does not correspond to where the
// cursor actually was, live and reproducibly, and standard DOM drag events
// never fire at all here (the webview's own native handler consumes the OS
// drag first). Identity-based matching needs no position data at all.
async function runAdd(paths: string[]): Promise<void> {
  showOperation(`Adding ${paths.length} file${paths.length === 1 ? "" : "s"}…`);
  try {
    const result = await api.addFiles(paths);
    hideOperation();
    await refreshList();
    reportAddResult(result);
    if (result.collisions.length > 0) {
      confirmReplaceBatch(result.collisions);
    }
  } catch (raw) {
    hideOperation();
    setStatus(describeForStatus("Couldn't add those files", api.describeError(raw)));
  }
}

function reportAddResult(result: api.AddResult): void {
  if (result.added.length === 0 && result.collisions.length === 0) {
    if (result.failed.length > 0) {
      setStatus(`Could not add ${result.failed.length} file${result.failed.length === 1 ? "" : "s"}.`);
    }
    return;
  }
  // FR-27/FR-9: the exact disclosure, once, on completion.
  const failedNote = result.failed.length > 0 ? ` ${result.failed.length} could not be added.` : "";
  setStatus(
    `Added ${result.added.length} file${result.added.length === 1 ? "" : "s"}. ` +
      `The originals are still on your disk — Veil doesn't delete them.${failedNote}`,
  );
}

function confirmReplaceBatch(collisions: api.Collision[]): void {
  const label =
    collisions.length === 1
      ? `Replace ${collisions[0]!.name}? Its current content in this vault will be gone.`
      : `Replace ${collisions.length} files already in this vault? Their current content will be gone.`;
  openModal(
    escapeHtml(label),
    [
      { label: "Cancel", run: () => {} },
      { label: "Replace", caution: true, run: () => void runReplaceBatch(collisions) },
    ],
    collisions.map((c) => (c.folder ? `${c.folder}/${c.name}` : c.name)),
  );
}

async function runReplaceBatch(collisions: api.Collision[]): Promise<void> {
  showOperation(`Replacing ${collisions.length} file${collisions.length === 1 ? "" : "s"}…`);
  let replaced = 0;
  const failed: string[] = [];
  for (const collision of collisions) {
    try {
      await api.replaceEntry(collision.folder, collision.name, collision.path);
      replaced += 1;
    } catch (raw) {
      failed.push(`${collision.name}: ${api.describeError(raw).message}`);
    }
  }
  hideOperation();
  await refreshList();
  const failedNote = failed.length > 0 ? ` ${failed.length} could not be replaced.` : "";
  setStatus(`Replaced ${replaced} file${replaced === 1 ? "" : "s"}.${failedNote}`);
}

function setupDropTarget(): void {
  const dropEl = el<HTMLElement>("drop-affordance");
  void getCurrentWebview().onDragDropEvent((event) => {
    const payload = event.payload;
    if (screens.vault.hidden) {
      return;
    }
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
      void runAdd(payload.paths);
    }
  });
}

// --------------------------------------------------------------- modal ---

function openModal(
  body: string,
  actions: Array<{ label: string; caution?: boolean; run: () => void }>,
  listItems: string[] = [],
): void {
  el<HTMLElement>("modal-body").innerHTML = body;
  const listEl = el<HTMLElement>("modal-list");
  listEl.innerHTML = listItems.map((item) => `<div>${escapeHtml(item)}</div>`).join("");
  const actionsEl = el<HTMLElement>("modal-actions");
  actionsEl.innerHTML = "";
  for (const action of actions) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = action.label;
    // Every action here is either "Cancel" (dismiss, stays plain) or the
    // dialog's one forward action (Replace, Delete, Check, OK, …) — the
    // latter is always styled as the primary choice, filled `caution`
    // instead of `accent` when it is also the destructive one.
    if (action.label !== "Cancel") {
      button.classList.add("primary");
    }
    if (action.caution) {
      button.classList.add("caution");
    }
    button.addEventListener("click", () => {
      closeModal();
      action.run();
    });
    actionsEl.appendChild(button);
  }
  el<HTMLElement>("modal-overlay").hidden = false;
}

function closeModal(): void {
  el<HTMLElement>("modal-overlay").hidden = true;
}

// ------------------------------------------------------------- selection -

function clearSelection(): void {
  selectedIds = new Set();
  lastClickedId = null;
  list.setSelection(selectedIds);
  updateSelectionButtons();
}

// P8.3.d: Replace… only ever means one file's content, so it stays
// available only for exactly one selected row.
function updateSelectionButtons(): void {
  el<HTMLButtonElement>("replace-selected-button").disabled = selectedIds.size !== 1;
  el<HTMLButtonElement>("delete-selected-button").disabled = selectedIds.size === 0;
}

function selectedEntries(): EntryInfo[] {
  return allEntries.filter((e) => selectedIds.has(e.id));
}

function setupSelection(): void {
  el<HTMLElement>("list-spacer").addEventListener("click", (event) => {
    const target = event.target as HTMLElement;
    const groupHeader = target.closest<HTMLElement>(".group-header");
    if (groupHeader) {
      const key = groupHeader.dataset.key ?? "";
      if (collapsedGroups.has(key)) {
        collapsedGroups.delete(key);
      } else {
        collapsedGroups.add(key);
      }
      renderList();
      return;
    }
    const row = target.closest<HTMLElement>(".entry-row");
    const id = row?.dataset.id;
    if (id === undefined) {
      selectedIds = new Set();
      lastClickedId = null;
    } else {
      const kind: ClickKind = event.shiftKey ? "shift" : event.metaKey ? "cmd" : "plain";
      const next = nextSelection(
        { selectedIds, lastClickedId },
        list.entryIds(),
        Number(id),
        kind,
      );
      selectedIds = next.selectedIds;
      lastClickedId = next.lastClickedId;
    }
    list.setSelection(selectedIds);
    updateSelectionButtons();
  });
  window.addEventListener("keydown", (event) => {
    if (event.key === "Delete" || event.key === "Backspace") {
      const entries = selectedEntries();
      if (entries.length > 0) {
        confirmDelete(entries);
      }
    }
  });
  el<HTMLButtonElement>("replace-selected-button").addEventListener("click", () => {
    void replaceSelected();
  });
  el<HTMLButtonElement>("delete-selected-button").addEventListener("click", () => {
    const entries = selectedEntries();
    if (entries.length > 0) {
      confirmDelete(entries);
    }
  });
}

// ------------------------------------------------------------ delete ---

function confirmDelete(entries: EntryInfo[]): void {
  const label =
    entries.length === 1 && entries[0]
      ? `Delete ${entries[0].name}?`
      : `Delete ${entries.length} files?`;
  openModal(escapeHtml(label), [
    { label: "Cancel", run: () => {} },
    {
      label: "Delete",
      caution: true,
      run: () => void runDelete(entries),
    },
  ]);
}

async function runDelete(entries: EntryInfo[]): Promise<void> {
  for (const entry of entries) {
    try {
      await api.deleteEntry(entry.id);
    } catch (raw) {
      setStatus(describeForStatus(`Couldn't delete ${entry.name}`, api.describeError(raw)));
      return;
    }
  }
  clearSelection();
  await refreshList();
}

// ------------------------------------------------------------------ context menu -

// Design §3.5: the same five actions the controls bar and double-click
// already reach, reached instead from a right-click on the selection.
function openContextMenu(x: number, y: number, entries: EntryInfo[]): void {
  if (entries.length === 0) {
    return;
  }
  const only = entries.length === 1 ? entries[0] : undefined;
  const items: Array<{ label: string; caution?: boolean; run: () => void }> = [
    { label: "Save as…", run: () => void extractSelection(entries) },
  ];
  if (only) {
    items.push({ label: "Show details", run: () => showDetails(only) });
  }
  // Preview is absent, not disabled, for an unsupported type, an over-cap
  // entry, or a multi-row selection — a greyed-out item trains a person to
  // stop reading before deciding whether it applies (Design §3.5).
  if (only && isPreviewable(only)) {
    items.push({ label: "Preview", run: () => openPreview(only) });
  }
  if (only) {
    items.push({ label: "Replace…", run: () => void replaceSelected() });
  }
  items.push({ label: "Delete", caution: true, run: () => confirmDelete(entries) });

  const menu = el<HTMLElement>("context-menu");
  menu.innerHTML = "";
  for (const item of items) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = item.label;
    if (item.caution) {
      button.classList.add("caution");
    }
    button.addEventListener("click", () => {
      closeContextMenu();
      item.run();
    });
    menu.appendChild(button);
  }
  menu.style.left = `${x}px`;
  menu.style.top = `${y}px`;
  menu.hidden = false;
}

function closeContextMenu(): void {
  el<HTMLElement>("context-menu").hidden = true;
}

function setupContextMenu(): void {
  el<HTMLElement>("list-spacer").addEventListener("contextmenu", (event) => {
    const row = (event.target as HTMLElement).closest<HTMLElement>(".entry-row");
    if (!row) {
      return;
    }
    event.preventDefault();
    const id = Number(row.dataset.id);
    // P8.4.a: right-clicking a row already inside the selection opens the
    // menu on that selection unchanged; right-clicking outside it replaces
    // the selection with just the clicked row first.
    if (!selectedIds.has(id)) {
      selectedIds = new Set([id]);
      lastClickedId = id;
      list.setSelection(selectedIds);
      updateSelectionButtons();
    }
    openContextMenu(event.clientX, event.clientY, selectedEntries());
  });
  document.addEventListener("mousedown", (event) => {
    const menu = el<HTMLElement>("context-menu");
    if (!menu.hidden && !menu.contains(event.target as Node)) {
      closeContextMenu();
    }
  });
  window.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !el<HTMLElement>("context-menu").hidden) {
      closeContextMenu();
    }
  });
}

// P8.5 replaces this stub with the real details panel (Design §8.9).
function showDetails(entry: EntryInfo): void {
  void entry;
}

// P8.6 replaces this stub with the real preview overlay (Design §8.10).
function openPreview(entry: EntryInfo): void {
  void entry;
}

// ------------------------------------------------------------ replace ---

// The explicit path (Design §8.7): select an entry, choose *any* file as
// its new content regardless of that file's own name — unlike the
// identity-matched replace in `runAdd`, this does not require the new
// file to share the old one's name.
async function replaceSelected(): Promise<void> {
  const entries = selectedEntries();
  const entry = entries.length === 1 ? entries[0] : undefined;
  if (!entry) {
    return;
  }
  const paths = await api.chooseSourcePaths(false);
  const sourcePath = paths[0];
  if (sourcePath === undefined) {
    return;
  }
  confirmReplace(entry, sourcePath);
}

function confirmReplace(entry: EntryInfo, sourcePath: string): void {
  openModal(
    `Replace ${escapeHtml(entry.name)}? Its current content in this vault will be gone.`,
    [
      { label: "Cancel", run: () => {} },
      {
        label: "Replace",
        caution: true,
        run: () => void runReplace(entry, sourcePath),
      },
    ],
  );
}

async function runReplace(entry: EntryInfo, sourcePath: string): Promise<void> {
  try {
    await api.replaceEntry(entry.folder, entry.name, sourcePath);
    await refreshList();
    setStatus(`Replaced ${entry.name}.`);
  } catch (raw) {
    setStatus(describeForStatus(`Couldn't replace ${entry.name}`, api.describeError(raw)));
  }
}

// -------------------------------------------------------- check for damage

el<HTMLButtonElement>("identity-check").addEventListener("click", () => {
  const estimateSeconds = Math.max(1, Math.round(allEntries.reduce((s, e) => s + e.size, 0) / (200 * 1024 * 1024)));
  openModal(
    `Check ${allEntries.length} files for damage? This reads the whole vault — about ${estimateSeconds}s.`,
    [
      { label: "Cancel", run: () => {} },
      { label: "Check", run: () => void runCheck() },
    ],
  );
});

async function runCheck(): Promise<void> {
  showOperation("Checking for damage…");
  try {
    const report = await api.checkVault();
    hideOperation();
    await refreshList();
    if (report.failures.length === 0) {
      const partial = report.complete ? "" : " The check was stopped early, so this covers only what it reached.";
      openModal(`Checked ${report.checked} files. No damage found.${partial}`, [
        { label: "OK", run: () => {} },
      ]);
    } else {
      openModal(
        `<strong>${report.failures.length} files are damaged.</strong> ` +
          "Their data in this vault can't be recovered — Veil doesn't keep a spare copy. " +
          "If you have a backup, restore these files from it.",
        [{ label: "OK", run: () => {} }],
        report.failures.map((f) => (f.folder ? `${f.folder}/${f.name}` : f.name)),
      );
    }
  } catch (raw) {
    hideOperation();
    setStatus(describeForStatus("Couldn't finish checking", api.describeError(raw)));
  }
}

// ------------------------------------------------------- change password -

el<HTMLButtonElement>("identity-change-password").addEventListener("click", () => {
  openChangePasswordModal();
});

function openChangePasswordModal(): void {
  const body =
    '<input type="password" id="cp-current" placeholder="Current password" autocomplete="current-password" />' +
    '<input type="password" id="cp-new" placeholder="New password" autocomplete="new-password" />' +
    '<input type="password" id="cp-new-confirm" placeholder="Retype new password" autocomplete="new-password" />' +
    "<p>If you forget the new password, this vault is lost the same way it would be " +
    "with the old one — there is still no recovery.</p>" +
    '<p id="cp-hint" class="hint" hidden></p>';
  openModal(body, [
    { label: "Cancel", run: () => {} },
    { label: "Change password", run: () => void submitChangePassword() },
  ]);
}

async function submitChangePassword(): Promise<void> {
  const current = (document.getElementById("cp-current") as HTMLInputElement | null)?.value ?? "";
  const next = (document.getElementById("cp-new") as HTMLInputElement | null)?.value ?? "";
  const confirm = (document.getElementById("cp-new-confirm") as HTMLInputElement | null)?.value ?? "";
  if (next.length < MIN_PASSWORD_LENGTH || next !== confirm) {
    setStatus("The new password must be at least 12 characters and match its retype.");
    return;
  }
  try {
    await api.changePassword(current, next);
    setStatus("Password changed.");
  } catch (raw) {
    setStatus(describeForStatus("Couldn't change the password", api.describeError(raw)));
  }
}

// -------------------------------------------------------------- startup --

setupDropTarget();
setupSelection();
setupContextMenu();
showScreen("firstRun");

// First run (Design §8.1) shows exactly two choices and nothing else — no
// third button for the fixture, in dev builds or otherwise. The fixture
// (Phase 5's bypass, still useful for the rendering checks it was built
// for — P5.5, P6.4) is reachable only by calling `__loadFixture()` from
// the devtools console (`cargo tauri dev --features devtools`), which
// touches no DOM a screenshot or T6.4 would see. In a release build
// `open_fixture_vault` does not exist at all, so this just errors.
(window as unknown as { __loadFixture: () => void }).__loadFixture = () => {
  void (async () => {
    const summary = await api.openFixtureVault();
    currentPath = "Fixture Vault.veil";
    await enterVault(summary);
  })();
};

function escapeHtml(value: string): string {
  const div = document.createElement("div");
  div.textContent = value;
  return div.innerHTML;
}
