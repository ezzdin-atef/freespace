// Dev-only fake backend, so the UI can be clicked through in a plain browser (`npm run dev`, then open
// http://localhost:5173). Never bundled into the app: main.ts imports it only in dev mode outside Tauri.
import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { AppRow, CleanRequest, FindingRow, Group, LeftoverRow, Plan, ProjectRow, Report, ResetPick, SafetyKey, ScanSummary, UsageLevel, UsageRow } from "../lib/api";

const GB = 1024 ** 3;
const MB = 1024 ** 2;

const DRIVES = [
  { root: "C:\\", kind: "Local", total: 475.6 * GB, free: 41.5 * GB },
  { root: "G:\\", kind: "Local", total: 200 * GB, free: 39.4 * GB },
];

const CATEGORIES: [string, string, string, string, SafetyKey][] = [
  ["BuildArtifacts", "Build output & dev caches", "Rust target, .NET bin/obj, Next.js .next... Rebuilt on the next build.", "Rebuilt by your next build.", "safe"],
  ["NodeModules", "node_modules", "JavaScript dependencies of your projects.", "Run npm install in the project.", "safe"],
  ["DevCache", "Package manager caches", "npm, pip, cargo, NuGet... downloaded again when needed.", "Downloaded again when needed.", "safe"],
  ["BrowserCache", "Browser caches", "Chrome / Edge / Firefox caches. Close the browser first.", "Nothing to do - recreated automatically.", "safe"],
  ["LogFiles", "Log files", "*.log files outside system folders.", "Not needed - nothing to restore.", "review"],
  ["CrashDumps", "Crash dumps & error reports", "Memory dumps from crashed apps.", "Not needed - nothing to restore.", "safe"],
  ["UserTemp", "User temp files", "Temporary files left by apps and installers.", "Not needed - nothing to restore.", "safe"],
  ["LargeFiles", "Large files", "Files of 512 MB or more. Review each one.", "Only from your own backup.", "review"],
];

/** Deterministic pseudo-random numbers, so the mock looks the same on every reload. */
function rng(seed: number) {
  return () => {
    seed = (seed * 1664525 + 1013904223) % 2 ** 32;
    return seed / 2 ** 32;
  };
}

function makeFindings(): FindingRow[] {
  const rand = rng(7);
  const projects = ["web-shop", "api", "logcleaner", "dashboard", "mobile", "docs-site", "ml-lab", "game"];
  const rows: FindingRow[] = [];
  let id = 0;
  for (const [category, , , , safety] of CATEGORIES) {
    const n = category === "LargeFiles" ? 12 : category === "LogFiles" ? 160 : 40 + Math.floor(rand() * 220);
    for (let i = 0; i < n; i++) {
      const p = projects[Math.floor(rand() * projects.length)];
      const name =
        category === "LargeFiles" ? `backup-${i}.iso` : category === "LogFiles" ? `server-${i}.log` : category === "NodeModules" ? "node_modules" : `target-${i}`;
      const path = `C:\\Users\\demo\\projects\\${p}\\packages\\module-${i}\\${name}`;
      const size = category === "LargeFiles" ? (0.6 + rand() * 6) * GB : rand() ** 3 * 2 * GB + rand() * 40 * MB;
      const protectedRow = category === "LargeFiles" && i === 0;
      rows.push({
        id: id++,
        category,
        path,
        name,
        size: Math.round(size),
        files: Math.floor(rand() * 40000),
        safety: protectedRow ? "protected" : safety,
        note: protectedRow ? "Inside a .git folder - deleting it loses history." : `Mock note for ${name}.`,
        kind: category === "LargeFiles" || category === "LogFiles" ? "file" : "dir",
        mode: category === "UserTemp" ? "contents" : "whole",
      });
    }
  }
  return rows;
}

/** Groups the findings still left by their project folder, like Rust's `projects::analyze`. */
function makeProjects(findings: FindingRow[]): ProjectRow[] {
  const kinds: Record<string, string[]> = {
    "web-shop": ["Node.js"],
    api: ["Rust"],
    logcleaner: ["Python"],
    dashboard: ["Node.js"],
    mobile: ["Flutter", "Node.js"],
    "docs-site": ["Node.js"],
    "ml-lab": ["Python"],
    game: [".NET"],
  };
  const daysAgo = [1, 3, 800, 12, 210, 45, 400, 95];
  return Object.keys(kinds).map((name, id) => {
    const path = `C:\\Users\\demo\\projects\\${name}`;
    const artifacts = findings
      .filter((f) => f.path.startsWith(`${path}\\`) && ["BuildArtifacts", "NodeModules", "LogFiles"].includes(f.category))
      .slice(0, 30)
      .map((f) => ({
        id: f.id,
        path: f.path,
        rel: f.path.slice(path.length + 1),
        label: f.category === "NodeModules" ? "node_modules" : f.category === "LogFiles" ? "Log files" : "Build output & dev caches",
        size: f.size,
        files: f.files,
        safety: f.safety,
        note: f.safety === "safe" ? f.note : "Committed to git - it will show up as deleted files until you restore or commit it",
        restore: f.category === "NodeModules" ? "pnpm install" : f.safety === "safe" ? "Rebuilt by your next build." : `git restore ${f.name}`,
      }))
      .sort((a, b) => b.size - a.size);
    const cleanable = artifacts.reduce((n, a) => n + a.size, 0);
    return {
      id,
      name,
      path,
      kinds: kinds[name],
      size: cleanable + (id + 1) * 180 * MB,
      files: (id + 1) * 21_000,
      cleanable,
      last_active: Math.floor(Date.now() / 1000) - daysAgo[id] * 86400,
      dormant: daysAgo[id] > 90,
      branch: id === 2 ? null : id === 5 ? "feature/search-box" : "main",
      artifacts,
    };
  });
}

/** A fake folder tree: every folder's children are derived from its path, so drilling down is stable. */
function usageLevel(path: string, size: number): UsageLevel {
  const rand = rng([...path].reduce((n, c) => (n * 31 + c.charCodeAt(0)) % 100000, 1));
  const names = ["projects", "AppData", "Downloads", "Videos", "node_modules", "target", ".cache", "Documents", "backups", "games"];
  const n = path.split("\\").length > 6 ? 0 : 4 + Math.floor(rand() * 6);
  const weights = Array.from({ length: n }, () => rand() ** 2 + 0.02);
  const sum = weights.reduce((a, b) => a + b, 0) || 1;
  // One rotation for the whole level keeps sibling names unique, like a real directory.
  const offset = Math.floor(rand() * names.length);
  const children: UsageRow[] = weights
    .map((w, i) => {
      const name = names[(i + offset) % names.length];
      const childSize = Math.round((size * 0.9 * w) / sum);
      const safety: SafetyKey = name === "Documents" ? "protected" : name === "AppData" ? "review" : "safe";
      return {
        name,
        path: `${path}\\${name}`,
        size: childSize,
        files: Math.round(childSize / 400_000),
        has_children: path.split("\\").length < 6,
        tag: name === "node_modules" ? "node_modules" : name === "target" ? "Build output & dev caches" : null,
        safety,
        note: safety === "protected" ? "Profile folder - delete individual items inside it instead" : "Mock note.",
      };
    })
    .sort((a, b) => b.size - a.size);
  return { name: path.split("\\").at(-1) ?? path, path, size, files: Math.round(size / 400_000), children };
}

function logo(letter: string, color: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48"><rect width="48" height="48" rx="12" fill="${color}"/><text x="24" y="32" font-family="Segoe UI" font-size="24" font-weight="700" fill="white" text-anchor="middle">${letter}</text></svg>`;
  return `data:image/svg+xml;base64,${btoa(svg)}`;
}

export function install() {
  let findings = makeFindings();
  let settings = { start_in_tray: true, rescan_minutes: 60, last_root: "C:\\Users\\demo" as string | null };
  let scanId = 1;
  // Lets a dev console fire backend events, e.g. __mockEmit("rescan:due", { root: "C:\\" }).
  (window as unknown as { __mockEmit: typeof emit }).__mockEmit = emit;
  let report: Report | null = null;
  const groups = (): Group[] =>
    CATEGORIES.map(([category, label, description, restore]) => {
      const rows = findings.filter((f) => f.category === category);
      return { category, label, description, restore, total: rows.reduce((n, f) => n + f.size, 0), count: rows.length };
    })
      .filter((g) => g.count > 0)
      .sort((a, b) => b.total - a.total);

  function plan(req: CleanRequest): Plan {
    const rows = (req.ids ?? []).map((id) => findings.find((f) => f.id === id)).filter((f) => f !== undefined);
    const ok = rows.filter((f) => f.safety !== "protected");
    const folders = req.folders ?? [];
    return {
      count: ok.length + folders.length,
      bytes: ok.reduce((n, f) => n + f.size, 0) + folders.length * 5 * GB,
      caution: [...ok.filter((f) => f.safety !== "safe").map((f) => `${f.path}  (${f.note})`), ...folders.map((f) => `${f}  (picked by hand)`)],
      blocked: rows.filter((f) => f.safety === "protected").map((f) => ({ path: f.path, reason: "Marked Do not delete" })),
      warn: null,
      dism: !!req.dism,
      dism_reset_base: !!req.dism && !!req.dism_reset_base,
      hibernate_off: !!req.hibernate_off,
    };
  }

  /** Fakes a clean: a few progress ticks, then the done event; the cleaned rows leave the scan. */
  function clean(req: CleanRequest) {
    const p = plan(req);
    const ids = new Set(req.ids ?? []);
    const started = Date.now();
    let done = 0;
    const tick = setInterval(() => {
      done = Math.min(done + Math.ceil(p.count / 6), p.count);
      emit("clean:progress", {
        done,
        total: p.count,
        freed: Math.round((p.bytes * done) / Math.max(p.count, 1)),
        files: done * 1200,
        failed: 0,
        current: findings.find((f) => ids.has(f.id))?.path ?? "",
        elapsed_ms: Date.now() - started,
        app_id: null,
      });
      if (done < p.count) return;
      clearInterval(tick);
      findings = findings.filter((f) => !ids.has(f.id) || f.safety === "protected");
      report = {
        freed: p.bytes,
        files: p.count * 1200,
        failed: 3,
        failures: [{ path: String.raw`C:\Users\demo\AppData\Local\Temp\locked.tmp`, reason: "In use by another process" }],
        skipped: p.blocked,
        maintenance: [p.dism ? "Component store cleanup finished." : "", p.hibernate_off ? "Hibernation turned off." : ""].filter(Boolean),
        processed: p.count,
        elapsed_ms: Date.now() - started,
        used_trash: !!req.use_trash,
        cancelled: false,
      };
      emit("clean:done", { report, app_id: null, drives: DRIVES });
    }, 350);
  }
  const scan: ScanSummary = {
    scan_id: 1,
    root: "C:\\Users\\demo",
    files: 2_310_000,
    dirs: 312_000,
    errors: 14,
    elapsed_ms: 20_800,
    cancelled: false,
  };
  const apps: AppRow[] = ["Visual Studio Code", "Slack", "Zoom", "7-Zip", "Node.js", "Old Toolbar"].map((name, id) => ({
    id,
    name,
    publisher: id === 2 ? "Zoom Video" : "Mock Inc.",
    version: "1.0",
    installed: ["2025-03-14", "2026-02-01", "", "2024-11-30", "2026-09-12", "2023-06-05"][id],
    location: id === 2 ? null : "C:\\Program Files\\" + name,
    size: (id + 1) * 300 * MB,
    store: id === 2,
    // The last one has lost its uninstaller, to exercise "Remove from installed list".
    has_uninstaller: id !== 5,
    removed: false,
  }));
  const leftovers: Record<number, { gen: number; items: LeftoverRow[] }> = {};
  let gen = 100;

  function scanLeftovers(appId: number) {
    const name = apps[appId].name;
    setTimeout(() => {
      leftovers[appId] = {
        gen: ++gen,
        items: [
          { id: 0, path: "C:\\Users\\demo\\AppData\\Roaming\\" + name, what: "Settings folder", size: 120 * MB, files: 830, safety: "safe", note: "", selected: true },
          { id: 1, path: "C:\\Users\\demo\\AppData\\Local\\" + name + "\\Cache", what: "Cache", size: 640 * MB, files: 5200, safety: "safe", note: "", selected: true },
          { id: 2, path: "C:\\ProgramData\\" + name, what: "Shared data", size: 30 * MB, files: 40, safety: "review", note: "Name only partly matches - check it really belongs to this app before deleting", selected: false },
        ],
      };
      emit("leftovers:changed", { app_id: appId });
    }, 300);
  }

  mockIPC(
    (cmd, args) => {
      switch (cmd) {
        case "init":
          return {
            drives: DRIVES,
            is_admin: false,
            scan_on_start: null,
            open_apps: false,
          };
        case "status":
          return { scan, scanning: null, cleaning: null, uninstalling: null, has_report: report !== null };
        case "groups":
          return groups();
        case "findings":
          return findings;
        case "get_settings":
          return settings;
        case "set_settings": {
          const a = args as { startInTray: boolean; rescanMinutes: number };
          settings = { ...settings, start_in_tray: a.startInTray, rescan_minutes: a.rescanMinutes };
          return settings;
        }
        case "start_scan": {
          const id = ++scanId;
          settings = { ...settings, last_root: (args as { root: string }).root };
          setTimeout(() => emit("scan:done", { scan_id: id, summary: { ...scan, scan_id: id } }), 400);
          return id;
        }
        case "stop_scan":
          return null;
        case "projects":
          return makeProjects(findings);
        case "full_clean_ids":
          return findings.filter((f) => f.safety === "safe" && f.category !== "LargeFiles").map((f) => f.id);
        case "plan_clean":
          return plan((args as { req: CleanRequest }).req);
        case "clean":
          clean((args as { req: CleanRequest }).req);
          return null;
        case "report":
          return report;
        case "stop_clean":
        case "reset_pc":
          return null;
        case "app_logos":
          // Coloured rounded squares with an initial, standing in for real icons (two apps keep the fallback).
          return Object.fromEntries(
            apps
              .filter((a) => a.id !== 2 && a.id !== 5)
              .map((a) => [a.id, logo(a.name[0], ["#2563eb", "#7c3aed", "#0891b2", "#16a34a", "#ea580c"][a.id % 5])]),
          );
        case "apps":
          // A fresh array each time, as real IPC gives (Svelte ignores reassigning the same one).
          return { loading: false, items: [...apps] };
        case "usage": {
          const path = (args as { path: string | null }).path;
          return usageLevel(path ?? scan.root, path ? Math.round(80 * GB * (path.length % 7 + 1) / 8) : 512 * GB);
        }
        case "scan_leftovers":
          scanLeftovers((args as { appId: number }).appId);
          return null;
        case "leftovers": {
          const l = leftovers[(args as { appId: number }).appId];
          return { loading: false, gen: l?.gen ?? null, items: l?.items ?? [] };
        }
        case "plan_leftovers":
        case "delete_leftovers": {
          const { appId, ids } = args as { appId: number; ids: number[] };
          const items = (leftovers[appId]?.items ?? []).filter((l) => ids.includes(l.id));
          const p: Plan = {
            count: items.length,
            bytes: items.reduce((n, l) => n + l.size, 0),
            caution: items.filter((l) => l.safety !== "safe").map((l) => `${l.path}  ${l.what} - ${l.note}`),
            blocked: [],
            warn: apps[appId].removed ? null : `${apps[appId].name} is still installed. Deleting its files will break it - uninstall it first.`,
            dism: false,
            dism_reset_base: false,
            hibernate_off: false,
          };
          if (cmd === "plan_leftovers") return p;
          setTimeout(() => {
            report = { freed: p.bytes, files: 900, failed: 0, failures: [], skipped: [], maintenance: [], processed: p.count, elapsed_ms: 400, used_trash: false, cancelled: false };
            emit("clean:done", { report, app_id: appId, drives: DRIVES });
            scanLeftovers(appId);
          }, 600);
          return null;
        }
        case "plan_reset":
        case "reset_apps": {
          // Only the fully matching settings folder counts as app data (like Rust's `reset_targets`).
          const picks = (args as { picks: ResetPick[] }).picks.filter((p) => !apps[p.app_id].removed);
          const items = picks.flatMap((p) => (leftovers[p.app_id]?.items ?? []).filter((l) => l.id === 0));
          const who = picks.length === 1 ? apps[picks[0].app_id].name : `these ${picks.length} apps`;
          const p: Plan = {
            count: items.length,
            bytes: items.reduce((n, l) => n + l.size, 0),
            caution: [],
            blocked: [],
            warn: items.length ? `Close ${who} first. Their settings, sign-ins and local data are deleted, and they start like a fresh install.` : null,
            dism: false,
            dism_reset_base: false,
            hibernate_off: false,
          };
          if (cmd === "plan_reset") return p;
          setTimeout(() => {
            report = { freed: p.bytes, files: 900, failed: 0, failures: [], skipped: [], maintenance: [], processed: p.count, elapsed_ms: 400, used_trash: false, cancelled: false };
            emit("clean:done", { report, app_id: picks.length === 1 ? picks[0].app_id : null, drives: DRIVES });
            for (const pk of picks) scanLeftovers(pk.app_id);
          }, 600);
          return null;
        }
        case "uninstall": {
          const appId = (args as { appId: number }).appId;
          emit("uninstall:progress", { app_id: appId, stage: "Follow the uninstaller window. Freespace waits until it is done." });
          setTimeout(() => {
            apps[appId] = { ...apps[appId], removed: true };
            emit("uninstall:done", { app_id: appId, removed: true, message: `${apps[appId].name} was uninstalled.` });
            scanLeftovers(appId);
          }, 1500);
          return null;
        }
        case "remove_entry": {
          const appId = (args as { appId: number }).appId;
          apps[appId] = { ...apps[appId], removed: true };
          return `Removed ${apps[appId].name} from the installed list.`;
        }
        case "stop_uninstall":
          return null;
        case "reveal":
          return null;
        case "plugin:dialog|open":
          return "D:\\projects";
        default:
          throw `The dev mock does not implement '${cmd}'.`;
      }
    },
    { shouldMockEvents: true },
  );
}
