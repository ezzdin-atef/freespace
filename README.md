# Freespace

A fast disk scanner and cleaner for Windows. Freespace reads a whole drive in parallel, groups what it finds
(temp files, caches, build output, `node_modules`, logs, crash dumps, Windows leftovers, large files), labels
every item **Safe**, **Review** or **Do not delete**, and only deletes what you tick and confirm.

**[Download the latest installer](https://github.com/ezzdin-atef/freespace/releases/latest/download/Freespace-setup.exe)** (Windows 10 and 11).
Installed copies update themselves.

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
- Icons: [Solar](https://icon-sets.iconify.design/solar/) (bold duotone), compiled into Svelte components at
  build time by `unplugin-icons`; every icon is picked in `ui/src/lib/icons.ts`.

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

Release build: the installer lands in `target/release/bundle/nsis/Freespace_<version>_x64-setup.exe` and the
portable exe in `target/release/freespace.exe`. The first build downloads Tauri's NSIS tools (needs internet).

```sh
npm run tauri build
```

Just the exe, no installer:

```sh
npm run tauri build -- --no-bundle
```

The installer is per-user (no admin prompt to install) and fetches the WebView2 runtime if it is missing.

Some cleanups (Windows temp, Windows Update cache, other users' files, most uninstalls) need the app to run
as administrator; the top bar offers to restart it elevated.

## Checks

```sh
cargo test               # Rust: scanner, safety rules, delete plans, a scan-plan-clean round trip
npm test                 # frontend unit tests (Vitest)
npm run check            # Svelte and TypeScript type check
```

## Releases

```sh
npm run release                     # asks for the next version (patch / minor / major / custom)
npm run release -- -Version 0.2.0   # no prompt
npm run release -- -DryRun          # validate only
```

`scripts/release.ps1` checks you are on a clean, up-to-date `main`, that `tauri.conf.json`, `Cargo.toml` and
`package.json` agree, and that the new version is valid semver, newer than the current one and not yet tagged.
It then bumps all version files (lockfiles too), runs the checks, commits `Release vX.Y.Z`, tags it and pushes
both. The tag starts `.github/workflows/release.yml`, which builds the NSIS installer, signs the update and
publishes a GitHub release with `latest.json`; the app checks that file on start and every 6 hours and offers
"Update to vX.Y.Z" in the top bar.

The landing page lives in `site/` and is deployed to GitHub Pages by `.github/workflows/pages.yml`. It reads the
latest release from the GitHub API, so it needs no redeploy per release.

One-time setup:

1. Generate the updater signing key (keep the private key secret; losing it means installed copies can no
   longer update): `npx tauri signer generate -w ~/.tauri/freespace.key`
2. Put the contents of `~/.tauri/freespace.key.pub` in `tauri.conf.json` > `plugins.updater.pubkey`.
3. In the GitHub repo, add the secrets `TAURI_SIGNING_PRIVATE_KEY` (contents of `~/.tauri/freespace.key`) and
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (empty if you set none).
4. Settings > Pages > Source: **GitHub Actions**.

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
scripts/        release.ps1: version bump, tag and push
site/           landing page for GitHub Pages
.github/        release and Pages workflows
docs/           design notes, including the migration from the earlier egui UI
```
