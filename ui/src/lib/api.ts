// Typed wrappers over the Rust commands in src/commands.rs. This is the only file that knows command and event names.
//
// Ids are only valid together with the id of the list they came from: finding ids with `scan_id`, leftover ids
// with `gen`. Rust refuses stale ones, so a rescan between showing and confirming can never delete the wrong thing.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type SafetyKey = "safe" | "review" | "protected";
export type KindKey = "dir" | "file" | "recycle_bin";

export interface Drive {
  root: string;
  kind: string;
  total: number;
  free: number;
}

export interface PathReason {
  path: string;
  reason: string;
}

// ------------------------------------------------------------------ startup and status

export interface Init {
  drives: Drive[];
  is_admin: boolean;
  scan_on_start: string | null;
  open_apps: boolean;
}

export interface Status {
  scan: ScanSummary | null;
  scanning: ScanProgress | null;
  cleaning: CleanProgress | null;
  uninstalling: UninstallProgress | null;
  has_report: boolean;
}

export const init = () => invoke<Init>("init");
export const status = () => invoke<Status>("status");

// ------------------------------------------------------------------ scan

export interface ScanProgress {
  scan_id: number;
  root: string;
  files: number;
  dirs: number;
  bytes: number;
  errors: number;
  current: string;
  elapsed_ms: number;
  expected: number | null;
}

export interface ScanSummary {
  scan_id: number;
  root: string;
  files: number;
  dirs: number;
  errors: number;
  elapsed_ms: number;
  cancelled: boolean;
}

export interface ScanDone {
  scan_id: number;
  /** null when the scan thread died. */
  summary: ScanSummary | null;
}

export const startScan = (root: string) => invoke<number>("start_scan", { root });
export const stopScan = () => invoke<void>("stop_scan");

// ------------------------------------------------------------------ results

export interface Group {
  category: string;
  label: string;
  description: string;
  /** How to get it back after cleaning. */
  restore: string;
  total: number;
  count: number;
}

export interface FindingRow {
  id: number;
  category: string;
  path: string;
  name: string;
  size: number;
  files: number;
  safety: SafetyKey;
  note: string;
  kind: KindKey;
  /** "contents" empties the folder and keeps it; "whole" deletes the item. */
  mode: "contents" | "whole";
}

export interface UsageRow {
  name: string;
  path: string;
  size: number;
  files: number;
  has_children: boolean;
  tag: string | null;
  safety: SafetyKey;
  note: string;
}

export interface UsageLevel {
  name: string;
  path: string;
  size: number;
  files: number;
  children: UsageRow[];
}

export const groups = (scanId: number) => invoke<Group[]>("groups", { scanId });
export const findings = (scanId: number) => invoke<FindingRow[]>("findings", { scanId });
export const fullCleanIds = (scanId: number) => invoke<number[]>("full_clean_ids", { scanId });
export const usage = (scanId: number, path: string | null = null) => invoke<UsageLevel>("usage", { scanId, path });

// ------------------------------------------------------------------ clean

export interface CleanRequest {
  scan_id: number;
  ids?: number[];
  /** Folders ticked in Space usage, exactly as `UsageRow.path` gave them. */
  folders?: string[];
  dism?: boolean;
  hibernate_off?: boolean;
  use_trash?: boolean;
}

export interface Plan {
  count: number;
  bytes: number;
  caution: string[];
  blocked: PathReason[];
  warn: string | null;
  dism: boolean;
  hibernate_off: boolean;
}

export interface CleanProgress {
  done: number;
  total: number;
  freed: number;
  files: number;
  failed: number;
  current: string;
  elapsed_ms: number;
  app_id: number | null;
}

export interface Report {
  freed: number;
  files: number;
  failed: number;
  failures: PathReason[];
  skipped: PathReason[];
  maintenance: string[];
  processed: number;
  elapsed_ms: number;
  used_trash: boolean;
  cancelled: boolean;
}

export interface CleanDone {
  /** null when the clean thread died. */
  report: Report | null;
  app_id: number | null;
  drives: Drive[];
}

export const planClean = (req: CleanRequest) => invoke<Plan>("plan_clean", { req });
export const clean = (req: CleanRequest) => invoke<void>("clean", { req });
export const stopClean = () => invoke<void>("stop_clean");
export const report = () => invoke<Report | null>("report");

// ------------------------------------------------------------------ installed apps

export interface AppRow {
  id: number;
  name: string;
  publisher: string;
  version: string;
  installed: string;
  location: string | null;
  size: number;
  store: boolean;
  has_uninstaller: boolean;
  removed: boolean;
}

export interface AppsList {
  loading: boolean;
  items: AppRow[];
}

export interface LeftoverRow {
  id: number;
  path: string;
  what: string;
  size: number;
  files: number;
  safety: SafetyKey;
  note: string;
  selected: boolean;
}

export interface Leftovers {
  loading: boolean;
  gen: number | null;
  items: LeftoverRow[];
}

export interface UninstallProgress {
  app_id: number;
  stage: string;
}

export interface UninstallDone {
  app_id: number;
  removed: boolean;
  message: string;
}

export const apps = () => invoke<AppsList>("apps");
export const scanLeftovers = (appId: number) => invoke<void>("scan_leftovers", { appId });
export const leftovers = (appId: number) => invoke<Leftovers>("leftovers", { appId });
export const planLeftovers = (appId: number, gen: number, ids: number[]) =>
  invoke<Plan>("plan_leftovers", { appId, gen, ids });
export const deleteLeftovers = (appId: number, gen: number, ids: number[], useTrash: boolean) =>
  invoke<void>("delete_leftovers", { appId, gen, ids, useTrash });
export const uninstall = (appId: number) => invoke<void>("uninstall", { appId });
export const stopUninstall = () => invoke<void>("stop_uninstall");
export const removeEntry = (appId: number) => invoke<string>("remove_entry", { appId });

// ------------------------------------------------------------------ system

export const reveal = (path: string) => invoke<void>("reveal", { path });
export const relaunchAdmin = () => invoke<boolean>("relaunch_admin");
export const resetPc = () => invoke<void>("reset_pc");

// ------------------------------------------------------------------ events

interface Events {
  "scan:progress": ScanProgress;
  "scan:done": ScanDone;
  "clean:progress": CleanProgress;
  "clean:done": CleanDone;
  "apps:changed": null;
  "leftovers:changed": { app_id: number };
  "uninstall:progress": UninstallProgress;
  "uninstall:done": UninstallDone;
}

/** Subscribes to a backend event; returns the unsubscribe function. */
export function on<E extends keyof Events>(event: E, handler: (payload: Events[E]) => void): Promise<UnlistenFn> {
  return listen<Events[E]>(event, (e) => handler(e.payload));
}
