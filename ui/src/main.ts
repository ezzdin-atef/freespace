import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";

// In a plain browser during development there is no Rust backend; fake one so the UI can be clicked through.
if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
  (await import("./dev/mock")).install();
}

export default mount(App, { target: document.getElementById("app")! });
