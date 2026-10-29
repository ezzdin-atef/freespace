# Freespace

A fast disk scanner and cleaner for Windows. Freespace reads a whole drive in parallel, groups what it finds
(temp files, caches, build output, `node_modules`, logs, crash dumps, Windows leftovers, large files), labels
every item **Safe**, **Review** or **Do not delete**, and only deletes what you tick and confirm.

## Features

- **Scan** a drive or a folder: millions of files in seconds, with live progress.
- **Clean up** by category, with a reason and a "how to get it back" note for every item. Protected items
  (Windows, installed programs, profile folders, `.git`) can never be deleted, and every delete is checked
  against the safety rules again in Rust before it runs.
- **Space usage**: browse folders biggest-first with a treemap, and tick any folder to delete it.
- **Large files**, **Full clean** (everything safe plus Windows rollback leftovers, with optional DISM and
  hibernation cleanup) and a **report** after each clean, with an optional Recycle Bin mode.
- **Installed apps**: uninstall desktop and Store apps, then delete the folders they leave behind.

## Tech

- Backend: Rust (`src/`) - scanner, cleaner, safety rules, installed apps, exposed as Tauri commands.
- Frontend: Svelte 5 + TypeScript + Vite (`ui/`), rendered by Tauri 2 / WebView2.
- Icons: [Phosphor](https://phosphoricons.com) (duotone).

## Requirements

- Windows 10 or 11 with the WebView2 runtime (built into Windows 11)
- [Rust](https://rustup.rs) (stable) with the MSVC toolchain
- [Node.js](https://nodejs.org) 20 or newer

## Development

```sh
npm install
npm run tauri dev        # the app with hot reload
npm run dev              # the UI alone in a browser, against a fake backend (ui/src/dev/mock.ts)
```

Release build (the exe lands in `target/release/freespace.exe`):

```sh
npm run tauri build -- --no-bundle
```

Some cleanups (Windows temp, Windows Update cache, other users' files, most uninstalls) need the app to run
as administrator; the top bar offers to restart it elevated.

## Checks

```sh
cargo test               # Rust: scanner, safety rules, delete plans, a scan-plan-clean round trip
npm test                 # frontend unit tests (Vitest)
npm run check            # Svelte and TypeScript type check
```

## Project layout

```
src/            Rust backend
  commands.rs   Tauri commands and progress events: the only surface the UI can call
  state.rs      session state and delete-plan building (ids are checked against the scan they came from)
  scanner.rs    parallel disk walk
  cleaner.rs    deletes with per-path safety re-checks
  safety.rs     what may and may not be deleted
  rules.rs      categories and matching rules
  apps.rs       installed apps, leftovers and uninstall
ui/src/         Svelte frontend
  lib/          API wrappers, session store, icons, formatting
  components/   buttons, table, dialogs, ...
  pages/        one file per page
icons/          app icons (generated from icons/source.svg with `npx tauri icon`)
capabilities/   Tauri permissions
docs/           design notes, including the migration from the earlier egui UI
```
