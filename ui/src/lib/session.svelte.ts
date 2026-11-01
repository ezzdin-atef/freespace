// App-wide state: mirrors the Rust session (drives, scan, progress) and holds pure UI state (view, toast).
// Rust stays the source of truth; this re-queries it after every `...:done` event.
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { SvelteMap, SvelteSet } from "svelte/reactivity";
import * as api from "./api";
import { fmtSize } from "./format";
import type {
  AppRow,
  CleanProgress,
  CleanRequest,
  Drive,
  FindingRow,
  Group,
  Leftovers,
  Plan,
  ProjectRow,
  Report,
  ResetPick,
  SafetyKey,
  ScanProgress,
  ScanSummary,
  Settings,
  UninstallProgress,
  UsageLevel,
  UsageRow,
} from "./api";

export type View =
  | { page: "home" }
  | { page: "cleanable"; category: string | null }
  | { page: "usage" }
  | { page: "large" }
  | { page: "full" }
  | { page: "report" }
  | { page: "projects" }
  | { page: "apps" };

export type Tone = "safe" | "review" | "danger";
export type Show = "all" | SafetyKey;
/** "date" (newest first) is offered only where rows have a date: Installed apps. */
export type Sort = "size" | "name" | "group" | "date";
export type AppShow = "all" | "desktop" | "store";
export type ProjectShow = "all" | "active" | "dormant";

/** What the confirm dialog is asking about. */
export type Confirm =
  | { kind: "clean"; plan: Plan; req: CleanRequest }
  | { kind: "leftovers"; plan: Plan; appId: number; gen: number; ids: number[] }
  | { kind: "reset"; plan: Plan; picks: ResetPick[] }
  | { kind: "uninstall"; appId: number }
  | { kind: "entry"; appId: number };

export interface Toast {
  id: number;
  tone: Tone;
  text: string;
}

const LARGE = "LargeFiles";

function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

class Session {
  drives = $state<Drive[]>([]);
  isAdmin = $state(false);
  /** Drive or folder the top bar's Scan button targets. */
  root = $state("C:\\");
  view = $state<View>({ page: "home" });

  scan = $state<ScanSummary | null>(null);
  scanning = $state<ScanProgress | null>(null);
  groups = $state<Group[]>([]);
  hasReport = $state(false);

  /** Findings of the current scan. Raw: rows are replaced as a whole, never mutated. */
  findings = $state.raw<FindingRow[]>([]);
  /** Ticked finding ids; only meaningful together with `scan.scan_id`. */
  readonly selected = new SvelteSet<number>();
  useTrash = $state(false);
  /** Finding ids "Full clean" would tick (decided in Rust). */
  fullIds = $state.raw<number[]>([]);
  /** Extra maintenance offered on the Full clean page. */
  dism = $state(false);
  /** DISM /ResetBase: only offered while `dism` is on. */
  dismResetBase = $state(false);
  hibernateOff = $state(false);

  confirm = $state<Confirm | null>(null);
  cleaning = $state<CleanProgress | null>(null);
  report = $state<Report | null>(null);

  // Space usage: one tree level at a time, loaded from Rust.
  usage = $state.raw<UsageLevel | null>(null);
  /** Folders opened below the scan root, outermost first. */
  usagePath = $state.raw<string[]>([]);
  usageFocus = $state<string | null>(null);
  /** Folders ticked in Space usage (path -> size); cleaned together with the ticked findings. */
  readonly selectedFolders = new SvelteMap<string, number>();

  /** Projects of the current scan. Their artifacts are findings, so they share `selected` and the clean flow. */
  projects = $state.raw<ProjectRow[]>([]);
  projectShow = $state<ProjectShow>("all");
  projectFocus = $state<number | null>(null);

  // Installed apps. App ids stay valid for the whole session (Rust loads the list once).
  apps = $state.raw<AppRow[]>([]);
  appsLoading = $state(true);
  /** App id -> logo (PNG data URL). Loaded in the background after the list; missing ids show a generic icon. */
  appLogos = $state.raw<Record<number, string>>({});
  appShow = $state<AppShow>("all");
  appFocus = $state<number | null>(null);
  leftovers = $state<Record<number, Leftovers>>({});
  /** Ticked leftover ids per app; reset to Rust's defaults whenever that app's list is rescanned. */
  leftoverTicks = $state<Record<number, number[]>>({});
  uninstalling = $state<UninstallProgress | null>(null);
  /** Apps ticked on the Installed apps page, to reset together. */
  readonly resetTicks = new SvelteSet<number>();
  /** True while the folders of apps about to be reset are still being looked up. */
  resetPreparing = $state(false);
  /** What the running clean was started for, so its done message fits. */
  private cleanKind: Confirm["kind"] | null = null;

  // List-page controls. Filter and sort carry over between pages; Show and focus reset.
  filter = $state("");
  sort = $state<Sort>("size");
  show = $state<Show>("all");
  focusId = $state<number | null>(null);

  settings = $state<Settings | null>(null);
  /** The running scan was started by the timer or tray, so it must not move the user to another page. */
  private quietScan = false;

  toast = $state<Toast | null>(null);
  private toastSeq = 0;
  private started = false;

  /** Cleanable groups, without large files (those have their own page). */
  readonly cleanGroups = $derived(this.groups.filter((g) => g.category !== LARGE));
  readonly cleanTotal = $derived(this.cleanGroups.reduce((n, g) => n + g.total, 0));
  readonly largeCount = $derived(this.groups.find((g) => g.category === LARGE)?.count ?? 0);
  readonly busy = $derived(this.scanning !== null || this.cleaning !== null || this.uninstalling !== null);
  readonly appCount = $derived(this.appsLoading && this.apps.length === 0 ? null : this.apps.length);

  readonly projectsCleanable = $derived(this.projects.reduce((n, p) => n + p.cleanable, 0));
  /** What "Tick safe in dormant" would tick. */
  readonly dormantSafeBytes = $derived(
    this.projects
      .filter((p) => p.dormant)
      .flatMap((p) => p.artifacts)
      .filter((a) => a.safety === "safe")
      .reduce((n, a) => n + a.size, 0),
  );

  readonly findingById = $derived(new Map(this.findings.map((f) => [f.id, f])));
  readonly groupByCategory = $derived(new Map(this.groups.map((g) => [g.category, g])));

  /** Count, bytes and how many are not marked Safe, over everything ticked. */
  readonly selection = $derived.by(() => {
    let count = 0;
    let bytes = 0;
    let review = 0;
    for (const id of this.selected) {
      const f = this.findingById.get(id);
      if (!f) continue;
      count++;
      bytes += f.size;
      if (f.safety !== "safe") review++;
    }
    // Hand-picked folders always count as "to review".
    for (const size of this.selectedFolders.values()) {
      count++;
      bytes += size;
      review++;
    }
    return { count, bytes, review };
  });

  notify(tone: Tone, text: string) {
    this.toast = { id: ++this.toastSeq, tone, text };
  }

  dismissToast() {
    this.toast = null;
  }

  go(view: View) {
    const same = view.page === this.view.page && JSON.stringify(view) === JSON.stringify(this.view);
    if (same) return;
    this.view = view;
    this.show = "all";
    this.focusId = null;
  }

  // ---------------------------------------------------------------- selection

  toggle(id: number) {
    const f = this.findingById.get(id);
    if (!f || f.safety === "protected") return;
    if (this.selected.has(id)) this.selected.delete(id);
    else this.selected.add(id);
  }

  /** Ticks every Safe finding in `category` (all cleanable categories when null). */
  selectSafe(category: string | null) {
    for (const f of this.findings) {
      const inView = category ? f.category === category : f.category !== LARGE;
      if (inView && f.safety === "safe") this.selected.add(f.id);
    }
  }

  /** Ticks or unticks the given findings (protected ones are never ticked). */
  setTicked(rows: FindingRow[], on: boolean) {
    for (const f of rows) {
      if (!on) this.selected.delete(f.id);
      else if (f.safety !== "protected") this.selected.add(f.id);
    }
  }

  setFolders(rows: UsageRow[], on: boolean) {
    for (const r of rows) {
      if (!on) this.selectedFolders.delete(r.path);
      else if (r.safety !== "protected") this.selectedFolders.set(r.path, r.size);
    }
  }

  selectNone() {
    this.selected.clear();
    this.selectedFolders.clear();
  }

  toggleFolder(row: UsageRow) {
    if (row.safety === "protected") return;
    if (this.selectedFolders.has(row.path)) this.selectedFolders.delete(row.path);
    else this.selectedFolders.set(row.path, row.size);
  }

  /** How much of this project's artifacts is ticked. */
  projectTicks(p: ProjectRow): "none" | "some" | "all" {
    const ticked = p.artifacts.filter((a) => this.selected.has(a.id)).length;
    return ticked === 0 ? "none" : ticked === p.artifacts.length ? "all" : "some";
  }

  toggleProject(p: ProjectRow) {
    this.setProjects([p], this.projectTicks(p) !== "all");
  }

  /** Ticks or unticks all the artifacts of these projects. The confirm dialog lists any that need a look. */
  setProjects(rows: ProjectRow[], on: boolean) {
    for (const p of rows) {
      for (const a of p.artifacts) {
        if (!on) this.selected.delete(a.id);
        else if (a.safety !== "protected") this.selected.add(a.id);
      }
    }
  }

  tickDormantSafe() {
    for (const p of this.projects) {
      if (!p.dormant) continue;
      for (const a of p.artifacts) if (a.safety === "safe") this.selected.add(a.id);
    }
  }

  // ---------------------------------------------------------------- cleaning

  /** Asks Rust what cleaning the ticked rows would do, then opens the confirm dialog. */
  async requestClean(maintenance = false) {
    if (!this.scan || this.busy || this.confirm) return;
    const req: CleanRequest = {
      scan_id: this.scan.scan_id,
      ids: [...this.selected],
      folders: [...this.selectedFolders.keys()],
      dism: maintenance && this.dism,
      dism_reset_base: maintenance && this.dism && this.dismResetBase,
      hibernate_off: maintenance && this.hibernateOff,
    };
    if (req.ids!.length === 0 && req.folders!.length === 0 && !req.dism && !req.hibernate_off) return;
    try {
      const plan = await api.planClean(req);
      if (plan.count === 0 && !plan.dism && !plan.hibernate_off) {
        this.notify("review", "Nothing in this selection can be cleaned - every item is protected.");
        return;
      }
      this.confirm = { kind: "clean", plan, req };
    } catch (e) {
      this.notify("danger", errorText(e));
    }
  }

  cancelConfirm() {
    this.confirm = null;
  }

  async confirmGo() {
    const c = this.confirm;
    if (!c) return;
    this.confirm = null;
    const placeholder = (total: number, app_id: number | null): CleanProgress => ({
      done: 0,
      total,
      freed: 0,
      files: 0,
      failed: 0,
      current: "",
      elapsed_ms: 0,
      app_id,
    });
    this.cleanKind = c.kind;
    try {
      switch (c.kind) {
        case "reset":
          await api.resetApps(c.picks, this.useTrash);
          this.cleaning = placeholder(c.plan.count, c.picks.length === 1 ? c.picks[0].app_id : null);
          break;
        case "clean":
          await api.clean({ ...c.req, use_trash: this.useTrash });
          this.cleaning = placeholder(c.plan.count, null);
          break;
        case "leftovers":
          await api.deleteLeftovers(c.appId, c.gen, c.ids, this.useTrash);
          this.cleaning = placeholder(c.plan.count, c.appId);
          break;
        case "uninstall":
          await api.uninstall(c.appId);
          this.uninstalling = { app_id: c.appId, stage: "" };
          break;
        case "entry":
          this.notify("safe", await api.removeEntry(c.appId));
          await this.refreshApps();
          break;
      }
    } catch (e) {
      this.notify("danger", errorText(e));
    }
  }

  async stopClean() {
    await api.stopClean();
  }

  private async cleanDone(d: api.CleanDone) {
    this.cleaning = null;
    this.drives = d.drives;
    if (!d.report) {
      this.notify("danger", "Cleaning stopped unexpectedly.");
    } else {
      this.report = d.report;
      this.hasReport = true;
    }
    if (this.cleanKind === "reset") {
      // Rust rescans each reset app's folders and sends `leftovers:changed`.
      this.cleanKind = null;
      this.resetTicks.clear();
      if (d.report) {
        const failed = d.report.failed > 0 ? ` ${d.report.failed} files were in use - close the apps and reset again.` : "";
        this.notify(d.report.failed > 0 ? "review" : "safe", `Reset done: deleted ${fmtSize(d.report.freed)} of app data.${failed}`);
      }
      return;
    }
    this.cleanKind = null;
    if (d.app_id !== null) {
      // Leftovers: Rust rescans the app's folders and sends `leftovers:changed`.
      if (d.report) {
        const failed = d.report.failed > 0 ? `, ${d.report.failed} files could not be deleted` : "";
        this.notify("safe", `Deleted ${fmtSize(d.report.freed)} of leftovers${failed}.`);
      }
      return;
    }
    // Cleaned findings drop out of the Rust session; reload so totals and the table match.
    this.selectedFolders.clear();
    if (this.scan) await this.loadScan(this.scan);
    for (const id of [...this.selected]) if (!this.findingById.has(id)) this.selected.delete(id);
    if (this.usage) await this.loadUsage(this.usagePath.at(-1) ?? null);
    if (d.report) this.go({ page: "report" });
  }

  /** Ticks what Full clean picks (replacing the selection); with `run`, also asks to clean them. */
  async fullClean(run: boolean) {
    this.selected.clear();
    for (const id of this.fullIds) this.selected.add(id);
    if (run) await this.requestClean(true);
    else this.go({ page: "cleanable", category: null });
  }

  async resetPc() {
    await api.resetPc().catch((e) => this.notify("danger", errorText(e)));
  }

  reveal(f: FindingRow) {
    if (f.kind !== "recycle_bin") this.revealPath(f.path);
  }

  revealPath(path: string) {
    api.reveal(path).catch((e) => this.notify("danger", errorText(e)));
  }

  /** Opens Windows' folder picker; resolves to the chosen folder, or null when cancelled. */
  async pickFolder(start?: string): Promise<string | null> {
    try {
      const path = await openDialog({ directory: true, title: "Choose a folder to scan", defaultPath: start || undefined });
      return typeof path === "string" ? path : null;
    } catch (e) {
      this.notify("danger", errorText(e));
      return null;
    }
  }

  // ---------------------------------------------------------------- space usage

  /** Loads one level of the scan tree (the root when `path` is null). */
  async loadUsage(path: string | null) {
    if (!this.scan) return;
    try {
      this.usage = await api.usage(this.scan.scan_id, path);
    } catch (e) {
      this.notify("danger", errorText(e));
    }
  }

  async openFolder(path: string) {
    this.usagePath = [...this.usagePath, path];
    this.usageFocus = null;
    await this.loadUsage(path);
  }

  /** Goes back to depth `n` (0 = the scan root), focusing the folder we came out of. */
  async usageUp(n: number) {
    if (n >= this.usagePath.length) return;
    const back = this.usagePath[n];
    this.usagePath = this.usagePath.slice(0, n);
    await this.loadUsage(this.usagePath.at(-1) ?? null);
    this.usageFocus = back;
  }

  // ---------------------------------------------------------------- installed apps

  focusApp(id: number) {
    this.appFocus = id;
    if (!(id in this.leftovers)) this.scanLeftovers(id);
  }

  async scanLeftovers(id: number) {
    const prev = this.leftovers[id];
    this.leftovers[id] = { gen: prev?.gen ?? null, items: prev?.items ?? [], loading: true };
    try {
      await api.scanLeftovers(id);
    } catch (e) {
      this.notify("danger", errorText(e));
    }
  }

  private async leftoversChanged(id: number) {
    const l = await api.leftovers(id);
    this.leftovers[id] = l;
    this.leftoverTicks[id] = l.items.filter((i) => i.selected && i.safety !== "protected").map((i) => i.id);
  }

  toggleLeftover(appId: number, id: number) {
    const ticks = this.leftoverTicks[appId] ?? [];
    this.leftoverTicks[appId] = ticks.includes(id) ? ticks.filter((t) => t !== id) : [...ticks, id];
  }

  async requestDeleteLeftovers(appId: number) {
    const l = this.leftovers[appId];
    const ids = this.leftoverTicks[appId] ?? [];
    if (!l || l.gen === null || ids.length === 0 || this.busy || this.confirm) return;
    try {
      const plan = await api.planLeftovers(appId, l.gen, ids);
      this.confirm = { kind: "leftovers", plan, appId, gen: l.gen, ids };
    } catch (e) {
      this.notify("danger", errorText(e));
    }
  }

  toggleResetTick(appId: number) {
    if (this.resetTicks.has(appId)) this.resetTicks.delete(appId);
    else this.resetTicks.add(appId);
  }

  setResetTicks(rows: AppRow[], on: boolean) {
    for (const a of rows) {
      if (!on) this.resetTicks.delete(a.id);
      else if (!a.removed) this.resetTicks.add(a.id);
    }
  }

  /** Finds each app's data folders (looking them up first where needed), then asks to reset them. */
  async requestReset(appIds: number[]) {
    if (appIds.length === 0 || this.busy || this.confirm || this.resetPreparing) return;
    this.resetPreparing = true;
    try {
      for (const id of appIds) if (!(id in this.leftovers)) this.scanLeftovers(id);
      // Each lookup ends with `leftovers:changed`; wait for all of them (they run in parallel).
      const deadline = Date.now() + 120_000;
      while (appIds.some((id) => this.leftovers[id]?.loading !== false)) {
        if (Date.now() > deadline) throw new Error("Looking up the apps' folders took too long. Try again.");
        await new Promise((r) => setTimeout(r, 150));
      }
      const picks: ResetPick[] = [];
      for (const id of appIds) {
        const gen = this.leftovers[id].gen;
        if (gen === null) throw new Error("Could not read the apps' folders. Try again.");
        picks.push({ app_id: id, gen });
      }
      const plan = await api.planReset(picks);
      if (plan.count === 0) {
        this.notify("review", "No data folders were found for the selected apps - nothing to reset.");
        return;
      }
      this.confirm = { kind: "reset", plan, picks };
    } catch (e) {
      this.notify("danger", errorText(e));
    } finally {
      this.resetPreparing = false;
    }
  }

  askUninstall(appId: number) {
    if (!this.busy) this.confirm = { kind: "uninstall", appId };
  }

  askRemoveEntry(appId: number) {
    if (!this.busy) this.confirm = { kind: "entry", appId };
  }

  async stopUninstall() {
    await api.stopUninstall();
  }

  /** Loads startup data, subscribes to backend events and restores any job already running. */
  async start() {
    if (this.started) return;
    this.started = true;

    await api.on("scan:progress", (p) => {
      if (!this.scanning || this.scanning.scan_id === p.scan_id) this.scanning = p;
    });
    await api.on("scan:done", (d) => this.scanDone(d));
    await api.on("apps:changed", () => this.refreshApps());
    await api.on("apps:logos", () => this.refreshLogos());
    await api.on("clean:progress", (p) => (this.cleaning = p));
    await api.on("clean:done", (d) => this.cleanDone(d));
    await api.on("leftovers:changed", (e) => this.leftoversChanged(e.app_id));
    await api.on("rescan:due", (e) => this.autoRescan(e.root));
    await api.on("rescan:now", () => this.startScan(this.settings?.last_root ?? this.root, true));
    await api.on("uninstall:progress", (p) => (this.uninstalling = p));
    await api.on("uninstall:done", (d) => {
      this.uninstalling = null;
      if (d.removed) this.notify("safe", `${d.message} Review what it left behind on the right.`);
      else this.notify("review", d.message);
      this.refreshApps();
    });

    try {
      const init = await api.init();
      this.drives = init.drives;
      this.isAdmin = init.is_admin;
      this.settings = await api.getSettings();
      this.root = this.settings.last_root ?? this.systemDrive()?.root ?? "C:\\";
      const st = await api.status();
      this.hasReport = st.has_report;
      this.scanning = st.scanning;
      this.cleaning = st.cleaning;
      this.uninstalling = st.uninstalling;
      if (st.has_report) this.report = await api.report();
      if (st.scan) await this.loadScan(st.scan);
      if (init.open_apps) this.view = { page: "apps" };
      else if (init.scan_on_start && !st.scanning) await this.startScan(init.scan_on_start);
    } catch (e) {
      this.notify("danger", `Could not start: ${errorText(e)}`);
    }
    await this.refreshApps();
    // Logos may have finished before this page subscribed to `apps:logos`.
    await this.refreshLogos();
  }

  systemDrive(): Drive | undefined {
    return this.drives.find((d) => d.root.toUpperCase() === "C:\\") ?? this.drives[0];
  }

  /** The drive `root` names, if it is a whole drive. */
  driveFor(root: string): Drive | undefined {
    const r = root.trim().toLowerCase();
    return this.drives.find((d) => d.root.toLowerCase() === r || d.root.toLowerCase() === `${r}\\`);
  }

  /** A rescan replaces the results, so it waits while the window is in use or anything is ticked or being confirmed.
   *  Rust asks again a minute later. */
  private autoRescan(root: string) {
    if (document.hasFocus() || this.confirm || this.resetPreparing || this.selection.count > 0) return;
    this.startScan(root, true);
  }

  async saveSettings(startInTray: boolean, rescanMinutes: number) {
    try {
      this.settings = await api.setSettings(startInTray, rescanMinutes);
    } catch (e) {
      this.notify("danger", errorText(e));
    }
  }

  async startScan(root = this.root, quiet = false) {
    if (this.busy) return;
    this.root = root;
    try {
      const id = await api.startScan(root);
      this.quietScan = quiet;
      if (this.settings) this.settings.last_root = root;
      // Show the scanning page at once instead of waiting for the first progress tick.
      this.scanning = {
        scan_id: id,
        root,
        files: 0,
        dirs: 0,
        bytes: 0,
        errors: 0,
        current: "",
        elapsed_ms: 0,
        expected: null,
      };
      this.scan = null;
      this.groups = [];
      this.findings = [];
      this.fullIds = [];
      this.projects = [];
      this.projectFocus = null;
      this.selected.clear();
      this.selectedFolders.clear();
      this.usage = null;
      this.usagePath = [];
      this.usageFocus = null;
      this.focusId = null;
      if (!quiet && this.view.page !== "apps") this.view = { page: "home" };
    } catch (e) {
      // Nobody asked for a quiet scan, so nobody needs to hear that it could not start.
      if (!quiet) this.notify("danger", errorText(e));
    }
  }

  async stopScan() {
    await api.stopScan();
  }

  private async scanDone(d: api.ScanDone) {
    if (this.scanning && this.scanning.scan_id !== d.scan_id) return;
    this.scanning = null;
    if (!d.summary) {
      this.notify("danger", "The scan stopped unexpectedly.");
      return;
    }
    if (d.summary.cancelled) this.notify("review", "Scan stopped - results are partial.");
    await this.loadScan(d.summary);
    const quiet = this.quietScan;
    this.quietScan = false;
    if (!quiet && this.view.page !== "apps") this.go({ page: "cleanable", category: null });
  }

  private async loadScan(summary: ScanSummary) {
    this.scan = summary;
    try {
      const id = summary.scan_id;
      const [groups, findings, fullIds, projects] = await Promise.all([
        api.groups(id),
        api.findings(id),
        api.fullCleanIds(id),
        api.projects(id),
      ]);
      this.projects = projects;
      this.groups = groups;
      this.findings = findings;
      this.fullIds = fullIds;
    } catch (e) {
      this.notify("danger", errorText(e));
    }
  }

  async relaunchAdmin() {
    try {
      if (!(await api.relaunchAdmin())) this.notify("review", "Windows did not start Freespace as administrator.");
    } catch (e) {
      this.notify("danger", errorText(e));
    }
  }

  private async refreshLogos() {
    try {
      this.appLogos = await api.appLogos();
    } catch {
      // Logos are decoration; the list works without them.
    }
  }

  private async refreshApps() {
    try {
      const list = await api.apps();
      this.apps = list.items;
      this.appsLoading = list.loading;
    } catch (e) {
      this.appsLoading = false;
      this.notify("danger", `Could not read the installed apps: ${errorText(e)}`);
    }
  }
}

export const session = new Session();
