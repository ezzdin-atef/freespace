import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";

// In a plain browser during development there is no Rust backend; fake one so the UI can be clicked through.
if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
  (await import("./dev/mock")).install();
}

// No browser context menu (Back, Reload, Inspect...) in the built app. Text fields keep theirs for copy and paste;
// dev builds keep it everywhere for Inspect.
if (!import.meta.env.DEV) {
  window.addEventListener("contextmenu", (e) => {
    const t = e.target as HTMLElement | null;
    if (!t?.closest("input, textarea, [contenteditable='true']")) e.preventDefault();
  });
}

export default mount(App, { target: document.getElementById("app")! });
