// Dev-only fake backend, so the UI can be clicked through in a plain browser (`npm run dev`, then open
// http://localhost:5173). Never bundled into the app: main.ts imports it only in dev mode outside Tauri.
import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { AppRow, CleanRequest, FindingRow, Group, Plan, Report, SafetyKey, ScanSummary } from "../lib/api";

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

export function install() {
  let findings = makeFindings();
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
    return {
      count: ok.length,
      bytes: ok.reduce((n, f) => n + f.size, 0),
      caution: ok.filter((f) => f.safety !== "safe").map((f) => `${f.path}  (${f.note})`),
      blocked: rows.filter((f) => f.safety === "protected").map((f) => ({ path: f.path, reason: "Marked Do not delete" })),
      warn: null,
      dism: !!req.dism,
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
  const apps: AppRow[] = ["Visual Studio Code", "Slack", "Zoom", "7-Zip", "Node.js"].map((name, id) => ({
    id,
    name,
    publisher: "Mock Inc.",
    version: "1.0",
    installed: "2026-01-01",
    location: null,
    size: (id + 1) * 300 * MB,
    store: id === 2,
    has_uninstaller: true,
    removed: false,
  }));

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
        case "apps":
          return { loading: false, items: apps };
        case "reveal":
          return null;
        default:
          throw `The dev mock does not implement '${cmd}'.`;
      }
    },
    { shouldMockEvents: true },
  );
}
