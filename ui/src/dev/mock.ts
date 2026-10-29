// Dev-only fake backend, so the UI can be clicked through in a plain browser (`npm run dev`, then open
// http://localhost:5173). Never bundled into the app: main.ts imports it only in dev mode outside Tauri.
import { mockIPC } from "@tauri-apps/api/mocks";
import type { AppRow, FindingRow, Group, SafetyKey, ScanSummary } from "../lib/api";

const GB = 1024 ** 3;
const MB = 1024 ** 2;

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
  const findings = makeFindings();
  const groups: Group[] = CATEGORIES.map(([category, label, description, restore]) => {
    const rows = findings.filter((f) => f.category === category);
    return { category, label, description, restore, total: rows.reduce((n, f) => n + f.size, 0), count: rows.length };
  }).sort((a, b) => b.total - a.total);
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
    (cmd) => {
      switch (cmd) {
        case "init":
          return {
            drives: [
              { root: "C:\\", kind: "Local", total: 475.6 * GB, free: 41.5 * GB },
              { root: "G:\\", kind: "Local", total: 200 * GB, free: 39.4 * GB },
            ],
            is_admin: false,
            scan_on_start: null,
            open_apps: false,
          };
        case "status":
          return { scan, scanning: null, cleaning: null, uninstalling: null, has_report: false };
        case "groups":
          return groups;
        case "findings":
          return findings;
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
