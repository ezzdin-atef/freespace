# LogCleaner: egui to Tauri 2 migration plan

Goal: replace the egui UI (`src/app.rs`, `src/ui_kit.rs`, ~3,500 lines) with a Tauri 2 + WebView2 frontend,
keeping every backend module unchanged. Reference look: `prototypes/tauri-home`.

## Decisions

| Topic | Choice | Why |
|---|---|---|
| Frontend | Svelte 5 + TypeScript + Vite | Least boilerplate, small bundle; fits a single-window utility |
| Styling | Plain CSS with tokens (from the prototype) | Already designed; no Tailwind dependency needed |
| Icons | Lucide (`lucide-svelte`) | Same stroke icons as the prototype |
| Big lists | `@tanstack/svelte-virtual` | Findings and apps lists can reach tens of thousands of rows |
| Treemap | Port `ui_kit::squarify` to TS (it is ~50 lines and already tested) | Keeps identical layout; avoids a d3 dependency |
| Layout | Rust stays at the repo root; frontend in `ui/`; `tauri.conf.json` at root | Minimal churn to existing modules |
| State owner | Rust | Scan tree and findings stay in Rust; the UI asks for what it shows |

## Trust boundary (important for a deletion tool)

- The UI never sends paths to delete. It sends **finding ids** (indices into the current scan) and options.
  Rust rebuilds the plan from its own `ScanResult` and re-runs `safety::deletion_blocked` before cleaning.
- Every scan gets a `scan_id`; clean/plan commands carry it and are refused if it is stale.
- Uninstall, leftovers and remove-entry take an app id from the Rust-side list, never a command line.
- CSP locked to `self`; no remote content; only `core:default` + our own commands in capabilities.

## Backend: new modules

`src/state.rs` — `Session` behind `tauri::State<Mutex<Session>>`: drives, current `ScanResult` + `scan_id`,
groups, apps list, leftovers cache, last `CleanReport`, running job handles (progress `Arc`s for cancel).

`src/commands.rs` — thin `#[tauri::command]` wrappers. Long jobs reuse the existing
`start(...) -> Receiver` + `Arc<Progress>` pattern: spawn a thread that emits a progress event every 100 ms
from the atomics and a `done` event when the receiver yields.

| Command | Replaces (Act / method) | Returns / emits |
|---|---|---|
| `init()` | `App::new` | drives, is_admin, CLI arg intent (`--apps` / path) |
| `start_scan(root)` | `start_scan`, `Act::Scan(Root)` | `scan_id`; events `scan:progress`, `scan:done` |
| `stop_scan()` | `Act::StopScan` | — |
| `groups()` | `regroup` | per-category totals for sidebar |
| `findings(filter)` | `visible_findings` | rows (id, category, path, size, files, safety, note, kind) |
| `usage_children(path)` | `usage_node`, `visible_usage` | one tree level (lazy, never the whole tree) |
| `build_plan(scan_id, ids, maint, use_trash, policy)` | `build_plan` | summary for the confirm modal |
| `clean(scan_id, ids, ...)` | `Act::ConfirmGo` | events `clean:progress`, `clean:done` (report) |
| `stop_clean()` | `Act::StopClean` | — |
| `report()` | `report_page` | last `CleanReport` |
| `apps()` | `apps::load` | events `apps:batch`, `apps:done` |
| `leftovers(app_id)` | `request_leftovers` | event `leftovers:done` |
| `uninstall(app_id)` / `stop_uninstall()` | `AskUninstall`, `StopUninstall` | events `uninstall:stage`, `uninstall:done` |
| `remove_entry(app_id)` | `AskRemoveEntry` | result |
| `delete_leftovers(app_id, ids)` | `DeleteLeftovers` | via clean events |
| `reveal(path)` / `relaunch_admin()` / `reset_pc()` | `Reveal`, `Admin`, `ResetPc` | — |

Pure UI state (view, filter text, sort, focus, ticked ids, Show/AppShow) moves to the frontend.

## Frontend structure (`ui/src`)

```
lib/api.ts          typed invoke/listen wrappers (one place that knows command names)
lib/stores.ts       session store: drives, scan status, groups, findings, selection, apps, toast
lib/format.ts       fmt_size / fmt_count ports
lib/squarify.ts     treemap layout
components/         Button, Select, Field, Pill, Card, NavItem, Table (virtual), Modal, Toast, Progress, Kbd
pages/              Home, Scanning, Findings (Cleanable + Large), Usage, FullClean, Report, Apps
App.svelte          shell: top bar, sidebar, page, detail pane, bulk bar, modals, keyboard shortcuts
```

## Phases (each ends with a working app)

0. **Safety net.** `git init` and commit the current egui app; work on a `tauri` branch.
1. **Scaffold.** Add Tauri + Vite/Svelte; `main.rs` boots Tauri; port tokens and base components from the prototype.
   egui code stays compiled-out but present until phase 7.
2. **Backend layer.** `state.rs` + `commands.rs` + event pump. Unit-test plan building and the id/scan_id checks
   without a UI.
3. **Shell, Home, Scanning.** Top bar with drive picker popup, sidebar, home page, live scan progress, stop scan, admin relaunch, CLI args.
4. **Findings.** Cleanable/Large pages: virtual table, sort, filter, Show tabs, tick/select-safe, detail pane,
   bulk bar, keyboard (Up/Down, Space, Del, Ctrl F, Esc).
5. **Cleaning.** Confirm modal (caution list, blocked list, trash + maintenance toggles), progress, cancel,
   report page, Full clean page.
6. **Usage and Apps.** Treemap + folder drill-down; installed apps list with filters, leftovers, uninstall,
   remove entry.
7. **Cut over.** Remove `app.rs`, `ui_kit.rs`, `eframe`, `egui-phosphor`; release build; check exe size,
   startup time, memory, and a full C: scan against the egui version.

## Risks

- **Large result sets.** Never send the whole `DirNode` tree; paginate or lazy-load. Measure `findings()` payload on a full C: scan.
- **Keyboard parity.** The egui app has focus handling in tables; needs deliberate work in phase 4.
- **WebView2 missing** on old Windows 10 installs; Tauri can bundle the bootstrapper if we ever ship an installer.
- **Toolchain.** Node is required from phase 1 (v24 is already installed).
