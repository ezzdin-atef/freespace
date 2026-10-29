import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// The frontend lives in ui/; Tauri loads the dev server in `tauri dev` and dist/ in release builds.
export default defineConfig({
  root: "ui",
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { outDir: "../dist", emptyOutDir: true, target: "es2022" },
});
