<div align="center">

<img src="icons/128x128@2x.png" alt="Freespace logo" width="112" height="112">

# Freespace

**A fast, safety-first disk scanner and cleaner for Windows.**

Scan a whole drive in seconds, see exactly what is eating your space, and reclaim it without the fear of
deleting the wrong thing.

[![Latest release](https://img.shields.io/github/v/release/ezzdin-atef/freespace?style=flat-square&color=2563eb)](https://github.com/ezzdin-atef/freespace/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/ezzdin-atef/freespace/total?style=flat-square&color=16a34a)](https://github.com/ezzdin-atef/freespace/releases)
[![Release build](https://img.shields.io/github/actions/workflow/status/ezzdin-atef/freespace/release.yml?style=flat-square&label=build)](https://github.com/ezzdin-atef/freespace/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/github/license/ezzdin-atef/freespace?style=flat-square&color=eab308)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-0078d4?style=flat-square)](#requirements)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-24c8db?style=flat-square)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-stable-b7410e?style=flat-square)](https://www.rust-lang.org)
[![Svelte](https://img.shields.io/badge/Svelte-5-ff3e00?style=flat-square)](https://svelte.dev)

[**Download for Windows**](https://github.com/ezzdin-atef/freespace/releases/latest/download/Freespace-setup.exe)
&nbsp;&middot;&nbsp;
[Website](https://ezzdin-atef.github.io/freespace/)
&nbsp;&middot;&nbsp;
[Releases](https://github.com/ezzdin-atef/freespace/releases)
&nbsp;&middot;&nbsp;
[Report a bug](https://github.com/ezzdin-atef/freespace/issues/new)

</div>

---

## Table of contents

- [Why Freespace](#why-freespace)
- [Features](#features)
- [What it cleans](#what-it-cleans)
- [How it keeps you safe](#how-it-keeps-you-safe)
- [Installation](#installation)
- [Building from source](#building-from-source)
- [Testing](#testing)
- [Releasing](#releasing)
- [Architecture](#architecture)
- [Contributing](#contributing)
- [Acknowledgements](#acknowledgements)
- [License](#license)

## Why Freespace

Most disk cleaners are either too timid to free real space or too aggressive to trust. Freespace reads your
whole drive in parallel, groups everything it finds into clear categories, and labels every single item
**Safe**, **Review** or **Do not delete**, with a reason and a note on how to get it back. Nothing is touched
until you tick it and confirm.

It is built for everyday users _and_ developers: alongside temp files and Windows leftovers, it finds the
gigabytes hiding in `node_modules`, build output, Python environments and tool caches.

## Features

| | |
| --- | --- |
| **Blazing-fast scan** | Walks millions of files in seconds across all CPU cores, with live progress. Scan a whole drive or a single folder. |
| **Smart cleanup** | Items grouped by category, each with a safety label, a reason, and a "how to get it back" note. |
| **Space usage explorer** | Browse folders biggest-first with an interactive treemap and delete any folder in one tick. |
| **Large file finder** | Surface the biggest files on the drive, wherever they hide. |
| **Full clean** | Everything safe, plus Windows rollback leftovers, with optional DISM component cleanup and hibernation file removal. |
| **App uninstaller** | Uninstall desktop and Microsoft Store apps, then sweep away the folders they leave behind. |
| **Recycle Bin mode** | Send items to the Recycle Bin instead of deleting them permanently. |
| **Clean report** | A summary after every clean of what was removed and how much space came back. |
| **Auto-updates** | Installed copies update themselves from signed GitHub releases. |

## What it cleans

| Area | Categories |
| --- | --- |
| **Windows** | Recycle Bin, user and system temp files, Windows Update cache, crash dumps and error reports, Windows logs, previous Windows installations, thumbnail cache |
| **Apps** | Browser caches, app caches, GPU shader caches, leftovers from uninstalled apps |
| **Developers** | `node_modules`, build artifacts, Python virtual environments, developer tool caches, log files |
| **You decide** | Large files, plus any folder you pick in the space usage view |

## How it keeps you safe

- **Three clear labels.** Every item is **Safe**, **Review** or **Do not delete**, so you always know the risk.
- **Protected paths stay protected.** Windows, installed programs, profile folders and `.git` directories can
  never be deleted, whatever you select.
- **Double-checked in Rust.** Every delete is checked against the safety rules a second time in the backend,
  right before it runs, and only for items that came from the current scan.
- **Nothing happens on its own.** Freespace deletes only what you tick and confirm, and Recycle Bin mode lets
  you undo.

## Installation

1. Download **[Freespace-setup.exe](https://github.com/ezzdin-atef/freespace/releases/latest/download/Freespace-setup.exe)**.
2. Run it. The installer is per-user, so it needs no administrator rights, and it fetches the WebView2 runtime
   if it is missing.
3. Launch Freespace and pick a drive to scan.

Some cleanups (Windows temp, the Windows Update cache, other users' files and most uninstalls) need
administrator rights. Freespace offers to restart itself elevated from the top bar when you need them.

### Requirements

- Windows 10 or Windows 11 (64-bit)
- Microsoft Edge WebView2 runtime (built into Windows 11, installed automatically otherwise)

## Building from source

### Prerequisites

- [Rust](https://rustup.rs) (stable) with the MSVC toolchain
- [Node.js](https://nodejs.org) 20 or newer
- Windows 10 or 11 with the WebView2 runtime

### Run in development

```sh
git clone https://github.com/ezzdin-atef/freespace.git
cd freespace
npm install
npm run tauri dev        # the full app with hot reload
```

Working on the UI only? `npm run dev` runs the frontend in a browser against a fake backend
(`ui/src/dev/mock.ts`), so no Rust build is needed.

### Production build

```sh
npm run tauri build                   # NSIS installer + portable exe
npm run tauri build -- --no-bundle    # portable exe only
```

| Output | Path |
| --- | --- |
| Installer | `target/release/bundle/nsis/Freespace_<version>_x64-setup.exe` |
| Portable exe | `target/release/freespace.exe` |

The first bundled build downloads Tauri's NSIS tools, so it needs an internet connection.

## Testing

```sh
cargo test               # Rust: scanner, safety rules, delete plans, a scan-plan-clean round trip
npm test                 # frontend unit tests (Vitest)
npm run check            # Svelte and TypeScript type check
```

## Releasing

```sh
npm run release                     # prompts for the next version (patch / minor / major / custom)
npm run release -- -Version 0.2.0   # no prompt
npm run release -- -DryRun          # validate only
```

`scripts/release.ps1` verifies that you are on a clean, up-to-date `main`, that `tauri.conf.json`,
`Cargo.toml` and `package.json` agree, and that the new version is valid semver, newer than the current one
and not yet tagged. It then bumps every version file (lockfiles included), runs the checks, commits
`Release vX.Y.Z`, tags it and pushes.

The tag triggers [`release.yml`](.github/workflows/release.yml), which builds the installer, signs the update
and publishes a GitHub release with `latest.json`. Installed apps check that file on start and every 6 hours
and offer "Update to vX.Y.Z" in the top bar.

The [landing page](https://ezzdin-atef.github.io/freespace/) lives in `site/` and is deployed by
[`pages.yml`](.github/workflows/pages.yml). It reads the latest release from the GitHub API, so it never needs
a redeploy per release.

<details>
<summary><strong>One-time setup for maintainers</strong></summary>

1. Generate the updater signing key. Keep the private key secret: losing it means installed copies can no
   longer update.
   ```sh
   npx tauri signer generate -w ~/.tauri/freespace.key
   ```
2. Put the contents of `~/.tauri/freespace.key.pub` in `tauri.conf.json` > `plugins.updater.pubkey`.
3. In the GitHub repo, add the secrets `TAURI_SIGNING_PRIVATE_KEY` (contents of `~/.tauri/freespace.key`) and
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (empty if you set none).
4. Under **Settings > Pages > Source**, choose **GitHub Actions**.

</details>

## Architecture

Freespace is a [Tauri 2](https://tauri.app) app: a Rust backend does all the disk work and exposes a small set
of commands, and a Svelte 5 + TypeScript frontend renders the UI in WebView2.

```
+---------------------------+        Tauri commands         +-----------------------------+
|  UI  (Svelte 5 + TS)      |  ------------------------->   |  Backend (Rust)             |
|  pages, treemap, dialogs  |  <-------------------------   |  scanner -> rules -> safety |
|                           |     progress events           |  cleaner, apps, updater     |
+---------------------------+                               +-----------------------------+
```

| Layer | Stack |
| --- | --- |
| Backend | Rust, [rayon](https://github.com/rayon-rs/rayon) for parallel scanning, Win32 APIs via `windows-sys` and `winreg`, [trash](https://github.com/Byron/trash-rs) for Recycle Bin mode |
| Frontend | Svelte 5, TypeScript, Vite |
| Shell | Tauri 2 / WebView2, with the updater and process plugins |
| Icons | [Solar](https://icon-sets.iconify.design/solar/) bold duotone, compiled to components at build time by `unplugin-icons` (picked in `ui/src/lib/icons.ts`) |

### Project layout

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

## Contributing

Contributions are welcome, from bug reports to new cleanup rules.

1. [Open an issue](https://github.com/ezzdin-atef/freespace/issues) to report a bug or discuss a feature
   before starting larger work.
2. Fork the repo and create a branch from `main`.
3. Make your change. New cleanup rules go in `src/rules.rs`; anything that affects what may be deleted
   belongs in `src/safety.rs` and needs a test.
4. Run `cargo test`, `npm test` and `npm run check`, and make sure they pass.
5. Open a pull request describing what changed and why.

Safety is the core promise of this app: changes that widen what can be deleted get extra scrutiny.

## Acknowledgements

- [Tauri](https://tauri.app) for the lightweight desktop shell
- [Svelte](https://svelte.dev) and [Vite](https://vitejs.dev) for the frontend
- [rayon](https://github.com/rayon-rs/rayon) for effortless parallelism
- [Solar icons](https://icon-sets.iconify.design/solar/) by 480 Design

## License

Freespace is released under the [MIT License](LICENSE). Copyright (c) 2026 Ezzdin Atef.

<div align="center">

If Freespace saved you some space, consider giving it a star.

</div>
