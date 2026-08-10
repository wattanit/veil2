// Typed wrappers over the Tauri command layer (P6.0). One function per
// command, so a call site never spells out a command name or its argument
// shape more than once.
import { invoke } from "@tauri-apps/api/core";

export interface ErrorInfo {
  kind: string;
  message: string;
}

export interface VaultSummary {
  entryCount: number;
  access: "readWrite" | "readOnly";
  unreadableCount: number;
}

export interface EntryInfo {
  id: number;
  name: string;
  folder: string;
  size: number;
  addedAt: number;
  unreadable: boolean;
}

export interface Collision {
  path: string;
  name: string;
  folder: string;
}

export interface AddResult {
  added: EntryInfo[];
  collisions: Collision[];
  failed: string[];
}

export interface CheckFailure {
  id: number;
  name: string;
  folder: string;
  damage: string;
}

export interface CheckReport {
  checked: number;
  complete: boolean;
  failures: CheckFailure[];
}

// invoke() rejects with the raw ErrorInfo object (Tauri passes a command's
// Err value through as-is); this just gives that shape a name at the call
// site instead of `catch (error: any)` everywhere.
export function isErrorInfo(value: unknown): value is ErrorInfo {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    "message" in value
  );
}

export function describeError(error: unknown): ErrorInfo {
  if (isErrorInfo(error)) {
    return error;
  }
  return { kind: "Internal", message: String(error) };
}

export const openVault = (path: string, password: string): Promise<VaultSummary> =>
  invoke("open_vault", { path, password });

export const createVault = (path: string, password: string): Promise<VaultSummary> =>
  invoke("create_vault", { path, password });

export const chooseVaultPath = (mode: "open" | "create"): Promise<string | null> =>
  invoke("choose_vault_path", { mode });

export const listEntries = (): Promise<EntryInfo[]> => invoke("list_entries");

export const closeVault = (): Promise<void> => invoke("close_vault");

export const cancelOperation = (): Promise<void> => invoke("cancel_operation");

export const extractEntry = (id: number, destination: string): Promise<void> =>
  invoke("extract_entry", { id, destination });

export const addFiles = (paths: string[]): Promise<AddResult> =>
  invoke("add_files", { paths });

export const chooseSavePath = (suggestedName: string): Promise<string | null> =>
  invoke("choose_save_path", { suggestedName });

export const chooseSourcePaths = (multiple: boolean): Promise<string[]> =>
  invoke("choose_source_paths", { multiple });

export const deleteEntry = (id: number): Promise<void> => invoke("delete_entry", { id });

export const replaceEntry = (
  folder: string,
  name: string,
  sourcePath: string,
): Promise<EntryInfo> => invoke("replace_entry", { folder, name, sourcePath });

export const changePassword = (current: string, next: string): Promise<void> =>
  invoke("change_password", { current, new: next });

export const checkVault = (): Promise<CheckReport> => invoke("check_vault");

// Debug-only (absent from a release build's command set entirely — Phase
// 5's fixture bypass, kept for exercising the list/rendering screens
// without walking through the create flow every time).
export const openFixtureVault = (): Promise<VaultSummary> => invoke("open_fixture_vault");
