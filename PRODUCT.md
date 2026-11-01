# Freespace

## Platform

Windows desktop app (Tauri 2, Rust, Svelte 5), plus a static landing page in `site/` published to GitHub Pages.

## Users

Anyone on Windows 10 or 11 whose drive is full and who wants safe, understandable cleanup: temp files, caches, Windows
Update files, logs, large files and full app removal. It is a whole-disk cleaner first.

Developers are an important group with extra features: their drives fill with `node_modules`, build output, package caches
and dormant projects, and they read paths and commands comfortably. Do not present Freespace as a tool only for code.

## Purpose

Scan a whole drive in seconds, show what is taking the space, and delete only what the user ticks and confirms.

## Position

- Every item carries a safety label (Safe, Review, Do not delete), a reason, and how to get it back.
- Understands code projects: last activity, lockfiles, what is committed to git. Reads `.git` as files and never runs
  `git`, because a scan can reach repositories the user does not trust.
- Free and open source (MIT). No account, no ads, no telemetry. Everything happens on the device.
- Small native app, written in Rust.

## Product facts to preserve

- Name: Freespace. Author: Ezzdin Atef. Repository: github.com/ezzdin-atef/freespace.
- Logo: `site/icon.svg`.
- Download link is the fixed-name latest asset `Freespace-setup.exe`; the page reads version and size from the GitHub
  releases API and must work without it.
- Windows only. Do not imply macOS or Linux support.
- Runs as administrator through the Windows prompt, lives in the tray, rescans on a schedule (off, 30 minutes to 1 day).

## Evidence rules

- Page imagery is drawn, not screenshots. Sizes and project names in drawings are examples, and the drawings that show them say so.
- No invented customers, testimonials, download counts or benchmarks. The only speed figure with evidence so far is one
  scan of about 440,000 files in about 2.5 seconds on the author's machine.
