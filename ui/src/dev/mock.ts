// Dev-only fake backend, so the UI can be clicked through in a plain browser (`npm run dev`, then open
// http://localhost:5173). Never bundled into the app: main.ts imports it only in dev mode outside Tauri.
import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { AppRow, CleanRequest, FindingRow, Group, KindKey, LeftoverRow, Plan, ProjectRow, Report, ResetPick, SafetyKey, ScanSummary, UsageLevel, UsageRow } from "../lib/api";

const GB = 1024 ** 3;
const MB = 1024 ** 2;

const DRIVES = [
  { root: "C:\\", kind: "Local", total: 475.6 * GB, free: 41.5 * GB },
  { root: "G:\\", kind: "Local", total: 200 * GB, free: 39.4 * GB },
];

const CATEGORIES: [string, string, string, string][] = [
  ["BuildArtifacts", "Build output & dev caches", "Rust target, .NET bin/obj, Next.js .next... Rebuilt on the next build.", "Rebuilt by your next build."],
  ["NodeModules", "node_modules", "JavaScript dependencies of your projects.", "Run npm install in the project."],
  ["DevCache", "Package manager caches", "npm, pip, cargo, NuGet... downloaded again when needed.", "Downloaded again when needed."],
  ["PythonEnv", "Python virtual environments", "Project virtual environments.", "python -m venv .venv, then pip install -r requirements.txt."],
  ["WindowsUpdate", "Windows Update cache", "Downloaded update packages. Windows re-downloads what it still needs.", "Windows downloads it again if it still needs it."],
  ["BrowserCache", "Browser caches", "Chrome / Edge / Firefox caches. Close the browser first.", "Nothing to do - recreated automatically."],
  ["UserTemp", "User temp files", "Temporary files left by apps and installers.", "Not needed - nothing to restore."],
  ["CrashDumps", "Crash dumps & error reports", "Memory dumps from crashed apps.", "Not needed - nothing to restore."],
  ["LogFiles", "Log files", "*.log files outside system folders.", "Not needed - nothing to restore."],
  ["LargeFiles", "Large files", "Files of 512 MB or more. Review each one.", "Only from your own backup."],
];

const HOME = "C:\\Users\\demo";
const CODE = `${HOME}\\code`;
const COMMITTED = "Committed to git - it will show up as deleted files until you restore or commit it";
const NO_LOCK = "No lockfile - a reinstall may pick different package versions";
const PNPM = "Restore with pnpm install - exact versions are pinned in pnpm-lock.yaml";
const NPM = "Restore with npm ci - exact versions are pinned in package-lock.json";
const REBUILT = "Rebuilt by your next build.";

/** rel, category, size in MB, safety, note, how to get it back. */
type DemoArtifact = [string, string, number, SafetyKey, string, string];
interface DemoProject {
  name: string;
  kinds: string[];
  days: number;
  branch: string | null;
  ownMb: number;
  files: number;
  artifacts: DemoArtifact[];
}

const PROJECTS: DemoProject[] = [
  { name: "billing-api", kinds: ["Rust"], days: 3, branch: "main", ownMb: 96, files: 84_210, artifacts: [
    ["target", "BuildArtifacts", 6840, "safe", "Rust build output - rebuilt by cargo build", "cargo build"],
  ] },
  { name: "storefront", kinds: ["Node.js"], days: 1, branch: "main", ownMb: 214, files: 171_340, artifacts: [
    ["node_modules", "NodeModules", 1910, "safe", PNPM, "pnpm install"],
    [".next", "BuildArtifacts", 648, "safe", "JS framework cache - rebuilt on next dev / build", "pnpm build"],
    ["playwright-report", "BuildArtifacts", 122, "safe", "Test run output - recreated by the next run", "pnpm test:e2e"],
  ] },
  { name: "ml-lab", kinds: ["Python"], days: 402, branch: "main", ownMb: 420, files: 96_020, artifacts: [
    [".venv", "PythonEnv", 4310, "review", "Virtual env - recreate it and reinstall packages", "python -m venv .venv, then pip install -r requirements.txt"],
    ["__pycache__", "BuildArtifacts", 38, "safe", "Python cache - recreated automatically", "Recreated automatically."],
  ] },
  { name: "mobile-app", kinds: ["Flutter", "Node.js"], days: 214, branch: "release/2.4", ownMb: 182, files: 142_800, artifacts: [
    ["build", "BuildArtifacts", 2240, "safe", "Build output - rebuilt on next build", "flutter build"],
    ["node_modules", "NodeModules", 930, "safe", NPM, "npm ci"],
    [".dart_tool", "BuildArtifacts", 246, "safe", "Dart / Flutter tool cache", "flutter pub get"],
  ] },
  { name: "dashboard", kinds: ["Node.js"], days: 12, branch: "main", ownMb: 148, files: 203_500, artifacts: [
    ["node_modules", "NodeModules", 1420, "safe", PNPM, "pnpm install"],
    [".claude\\worktrees\\agent-1\\node_modules", "NodeModules", 1380, "safe", PNPM, "pnpm install"],
    [".pnpm-store", "BuildArtifacts", 980, "safe", "Project pnpm store - packages download again on the next install", "pnpm install"],
  ] },
  { name: "game-tools", kinds: [".NET"], days: 96, branch: "main", ownMb: 310, files: 31_700, artifacts: [
    ["bin", "BuildArtifacts", 1120, "safe", ".NET build output - rebuilt by dotnet build", "dotnet build"],
    [".vs", "BuildArtifacts", 424, "safe", "Visual Studio cache - recreated on next open", "Recreated on next open."],
    ["obj", "BuildArtifacts", 382, "safe", ".NET build output - rebuilt by dotnet build", "dotnet build"],
  ] },
  { name: "old-portfolio", kinds: ["Node.js"], days: 811, branch: null, ownMb: 64, files: 58_900, artifacts: [
    ["node_modules", "NodeModules", 1240, "review", NO_LOCK, "npm install"],
  ] },
  { name: "docs-site", kinds: ["Node.js"], days: 241, branch: "feature/search-box", ownMb: 88, files: 64_300, artifacts: [
    ["node_modules", "NodeModules", 1640, "safe", PNPM, "pnpm install"],
    ["dist", "BuildArtifacts", 312, "review", COMMITTED, "git restore dist"],
    [".astro", "BuildArtifacts", 96, "safe", "JS framework cache - rebuilt on next dev / build", "pnpm build"],
  ] },
];

/** category, path, size in MB, files, safety, note, kind, mode. */
const OTHER_FINDINGS: [string, string, number, number, SafetyKey, string, KindKey, "contents" | "whole"][] = [
  ["DevCache", `${HOME}\\AppData\\Local\\npm-cache`, 4210, 182_400, "safe", "npm cache", "dir", "contents"],
  ["DevCache", `${HOME}\\.cargo\\registry\\src`, 2760, 241_000, "safe", "Cargo extracted crate sources", "dir", "contents"],
  ["DevCache", `${HOME}\\.gradle\\caches`, 3180, 64_200, "review", "Gradle caches - next build downloads again", "dir", "contents"],
  ["DevCache", `${HOME}\\.nuget\\packages`, 2340, 92_700, "review", "NuGet packages - restored by dotnet restore", "dir", "contents"],
  ["DevCache", `${HOME}\\AppData\\Local\\pip\\cache`, 1120, 9_600, "safe", "pip cache", "dir", "contents"],
  ["WindowsUpdate", "C:\\Windows\\SoftwareDistribution\\Download", 5120, 1_840, "safe", "Downloaded Windows updates", "dir", "contents"],
  ["BrowserCache", `${HOME}\\AppData\\Local\\Google\\Chrome\\User Data\\Default\\Cache`, 1460, 22_100, "safe", "Chrome cache", "dir", "contents"],
  ["BrowserCache", `${HOME}\\AppData\\Local\\Microsoft\\Edge\\User Data\\Default\\Cache`, 720, 11_300, "safe", "Edge cache", "dir", "contents"],
  ["UserTemp", `${HOME}\\AppData\\Local\\Temp`, 2930, 38_400, "safe", "Temp folder (contents only)", "dir", "contents"],
  ["CrashDumps", `${HOME}\\AppData\\Local\\CrashDumps`, 860, 14, "safe", "App crash dumps", "dir", "contents"],
  ["LogFiles", `${CODE}\\billing-api\\logs\\server.log`, 410, 1, "review", "Written in the last 7 days - the app may still use it", "file", "whole"],
  ["LargeFiles", `${HOME}\\AppData\\Local\\Docker\\wsl\\disk\\docker_data.vhdx`, 41_300, 1, "review", "Virtual disk (WSL / Docker / VM) - deleting it destroys that machine's data", "file", "whole"],
  ["LargeFiles", `${HOME}\\Downloads\\ubuntu-24.04-desktop-amd64.iso`, 6010, 1, "review", "Disk image - safe to delete if you no longer need it", "file", "whole"],
  ["LargeFiles", `${HOME}\\Videos\\conference-talk-final.mp4`, 3640, 1, "review", "Video - your personal data", "file", "whole"],
  ["LargeFiles", `${CODE}\\ml-lab\\data\\embeddings-2024.zip`, 2210, 1, "review", "Archive - check whether you still need it", "file", "whole"],
  ["LargeFiles", `${CODE}\\storefront\\.git\\objects\\pack\\pack-28bb02ec.pack`, 1180, 1, "protected", "Git repository data - deleting it loses history", "file", "whole"],
  ["LargeFiles", "C:\\pagefile.sys", 16_384, 1, "protected", "Windows virtual memory - managed by Windows", "file", "whole"],
];

function makeFindings(): FindingRow[] {
  const rows: FindingRow[] = [];
  for (const p of PROJECTS) {
    for (const [rel, category, mb, safety, note] of p.artifacts) {
      const path = `${CODE}\\${p.name}\\${rel}`;
      rows.push({ id: rows.length, category, path, name: rel.split("\\").at(-1)!, size: mb * MB, files: Math.round((mb * MB) / 28_000), safety, note, kind: "dir", mode: "whole" });
    }
  }
  for (const [category, path, mb, files, safety, note, kind, mode] of OTHER_FINDINGS) {
    rows.push({ id: rows.length, category, path, name: path.split("\\").at(-1)!, size: mb * MB, files, safety, note, kind, mode });
  }
  return rows;
}

/** The demo projects with the artifacts still left, like Rust's `projects` command. */
function makeProjects(findings: FindingRow[]): ProjectRow[] {
  const now = Math.floor(Date.now() / 1000);
  return PROJECTS.map((p, id) => {
    const root = `${CODE}\\${p.name}`;
    const artifacts = p.artifacts.flatMap(([rel, category, , , , restore]) => {
      const path = `${root}\\${rel}`;
      const f = findings.find((x) => x.path === path);
      if (!f) return [];
      const label = CATEGORIES.find(([c]) => c === category)?.[1] ?? category;
      return [{ id: f.id, path, rel, label, size: f.size, files: f.files, safety: f.safety, note: f.note, restore }];
    });
    const cleanable = artifacts.reduce((n, a) => n + a.size, 0);
    return {
      id,
      name: p.name,
      path: root,
      kinds: p.kinds,
      size: cleanable + p.ownMb * MB,
      files: p.files,
      cleanable,
      last_active: now - p.days * 86400,
      dormant: p.days > 90,
      branch: p.branch,
      artifacts,
    };
  });
}

/** Deterministic pseudo-random numbers, so the mock looks the same on every reload. */
function rng(seed: number) {
  return () => {
    seed = (seed * 1664525 + 1013904223) % 2 ** 32;
    return seed / 2 ** 32;
  };
}

/** A fake folder tree: every folder's children are derived from its path, so drilling down is stable. */
function usageLevel(path: string, size: number): UsageLevel {
  // The top level matches the demo projects' total; deeper levels are made up from the path.
  const top: Record<string, number> = { code: 27.1, AppData: 74, Downloads: 38, Videos: 52, ".cargo": 6, ".gradle": 9, go: 4, Documents: 21, backups: 44, Pictures: 11 };
  const isTop = path === HOME;
  const rand = rng([...path].reduce((n, c) => (n * 31 + c.charCodeAt(0)) % 100000, 1));
  const names = ["code", "AppData", "Downloads", "Videos", ".cargo", ".gradle", "go", "Documents", "backups", "Pictures"];
  const n = isTop ? names.length : path.split("\\").length > 6 ? 0 : 4 + Math.floor(rand() * 6);
  const weights = Array.from({ length: n }, () => rand() ** 2 + 0.02);
  const sum = weights.reduce((a, b) => a + b, 0) || 1;
  // One rotation for the whole level keeps sibling names unique, like a real directory.
  const offset = Math.floor(rand() * names.length);
  const children: UsageRow[] = weights
    .map((w, i) => {
      const name = names[(i + offset) % names.length];
      const childSize = isTop ? top[name] * GB : Math.round((size * 0.9 * w) / sum);
      const safety: SafetyKey = ["Documents", "Downloads", "Videos", "Pictures"].includes(name) ? "protected" : "review";
      return {
        name,
        path: `${path}\\${name}`,
        size: childSize,
        files: Math.round(childSize / 400_000),
        has_children: path.split("\\").length < 6,
        tag: null,
        safety,
        note: safety === "protected" ? "Profile folder - delete individual items inside it instead" : "Your files - review before deleting",
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
    publisher: ["Microsoft", "Slack Technologies", "Zoom Video", "Igor Pavlov", "OpenJS Foundation", "Unknown"][id],
    version: ["1.96.2", "4.41.105", "6.3.0", "24.09", "22.12.0", "2.1"][id],
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
            is_admin: true,
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
          return usageLevel(path ?? scan.root, path ? Math.round(40 * GB * (path.length % 7 + 1) / 8) : 286 * GB);
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
