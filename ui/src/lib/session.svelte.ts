// App-wide state: mirrors the Rust session (drives, scan, progress) and holds pure UI state (view, toast).
// Rust stays the source of truth; this re-queries it after every `...:done` event.
import { SvelteSet } from "svelte/reactivity";
import * as api from "./api";
import type { Drive, FindingRow, Group, SafetyKey, ScanProgress, ScanSummary } from "./api";

export type View =
  | { page: "home" }
  | { page: "cleanable"; category: string | null }
  | { page: "usage" }
  | { page: "large" }
  | { page: "full" }
  | { page: "report" }
  | { page: "apps" };

export type Tone = "safe" | "review" | "danger";
export type Show = "all" | SafetyKey;
export type Sort = "size" | "name" | "group";

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
  appCount = $state<number | null>(null);

  /** Findings of the current scan. Raw: rows are replaced as a whole, never mutated. */
  findings = $state.raw<FindingRow[]>([]);
  /** Ticked finding ids; only meaningful together with `scan.scan_id`. */
  readonly selected = new SvelteSet<number>();
  useTrash = $state(false);

  // List-page controls. Filter and sort carry over between pages, like the egui app; Show and focus reset.
  filter = $state("");
  sort = $state<Sort>("size");
  show = $state<Show>("all");
  focusId = $state<number | null>(null);

  toast = $state<Toast | null>(null);
  private toastSeq = 0;
  private started = false;

  /** Cleanable groups, without large files (those have their own page). */
  readonly cleanGroups = $derived(this.groups.filter((g) => g.category !== LARGE));
  readonly cleanTotal = $derived(this.cleanGroups.reduce((n, g) => n + g.total, 0));
  readonly largeCount = $derived(this.groups.find((g) => g.category === LARGE)?.count ?? 0);
  readonly busy = $derived(this.scanning !== null);

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

  selectNone() {
    this.selected.clear();
  }

  /** Opens the confirm dialog for what is ticked. The clean flow moves over in migration phase 5. */
  requestClean() {
    if (this.selection.count === 0 || this.busy) return;
    this.notify("review", "Cleaning moves to the new interface in the next migration phase. Use the egui build to clean for now.");
  }

  reveal(f: FindingRow) {
    if (f.kind !== "recycle_bin") api.reveal(f.path).catch((e) => this.notify("danger", errorText(e)));
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

    try {
      const init = await api.init();
      this.drives = init.drives;
      this.isAdmin = init.is_admin;
      this.root = this.systemDrive()?.root ?? "C:\\";
      const st = await api.status();
      this.hasReport = st.has_report;
      this.scanning = st.scanning;
      if (st.scan) await this.loadScan(st.scan);
      if (init.open_apps) this.view = { page: "apps" };
      else if (init.scan_on_start && !st.scanning) await this.startScan(init.scan_on_start);
    } catch (e) {
      this.notify("danger", `Could not start: ${errorText(e)}`);
    }
    await this.refreshApps();
  }

  systemDrive(): Drive | undefined {
    return this.drives.find((d) => d.root.toUpperCase() === "C:\\") ?? this.drives[0];
  }

  /** The drive `root` names, if it is a whole drive. */
  driveFor(root: string): Drive | undefined {
    const r = root.trim().toLowerCase();
    return this.drives.find((d) => d.root.toLowerCase() === r || d.root.toLowerCase() === `${r}\\`);
  }

  async startScan(root = this.root) {
    if (this.busy) return;
    this.root = root;
    try {
      const id = await api.startScan(root);
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
      this.selected.clear();
      this.focusId = null;
      if (this.view.page !== "apps") this.view = { page: "home" };
    } catch (e) {
      this.notify("danger", errorText(e));
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
    if (this.view.page !== "apps") this.go({ page: "cleanable", category: null });
  }

  private async loadScan(summary: ScanSummary) {
    this.scan = summary;
    try {
      const [groups, findings] = await Promise.all([api.groups(summary.scan_id), api.findings(summary.scan_id)]);
      this.groups = groups;
      this.findings = findings;
    } catch (e) {
      this.notify("danger", errorText(e));
    }
  }

  async relaunchAdmin() {
    try {
      if (!(await api.relaunchAdmin())) this.notify("review", "Windows did not start LogCleaner as administrator.");
    } catch (e) {
      this.notify("danger", errorText(e));
    }
  }

  private async refreshApps() {
    try {
      const list = await api.apps();
      this.appCount = list.loading && list.items.length === 0 ? null : list.items.length;
    } catch {
      // The apps page reports load errors; the sidebar count can stay empty.
    }
  }
}

export const session = new Session();
