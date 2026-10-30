// Auto-update from GitHub Releases. Rust's updater plugin fetches latest.json (tauri.conf.json > plugins.updater),
// checks the signature against the bundled public key, and runs the NSIS installer in passive mode.
import { isTauri } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

/** Checks again this often while the app stays open. */
const RECHECK_MS = 6 * 60 * 60 * 1000;

class Updater {
  /** The newer release, once one is found. */
  available = $state<Update | null>(null);
  /** 0..1 while downloading, null otherwise. */
  progress = $state<number | null>(null);
  error = $state<string | null>(null);
  private started = false;

  /** Checks now and then every few hours. Silent when offline or outside Tauri (the browser dev mock). */
  start() {
    if (this.started || !isTauri()) return;
    this.started = true;
    this.check();
    setInterval(() => this.check(), RECHECK_MS);
  }

  async check() {
    if (this.available || this.progress !== null) return;
    try {
      this.available = await check();
    } catch (e) {
      console.warn("update check failed", e);
    }
  }

  /** Downloads, installs and restarts. On Windows the installer closes the app itself before relaunch runs. */
  async install() {
    const update = this.available;
    if (!update || this.progress !== null) return;
    this.error = null;
    this.progress = 0;
    let total = 0;
    let done = 0;
    try {
      await update.downloadAndInstall((e) => {
        if (e.event === "Started") total = e.data.contentLength ?? 0;
        else if (e.event === "Progress") {
          done += e.data.chunkLength;
          this.progress = total ? done / total : 0;
        } else this.progress = 1;
      });
      await relaunch();
    } catch (e) {
      this.progress = null;
      this.error = `Update failed: ${e}`;
    }
  }
}

export const updater = new Updater();
