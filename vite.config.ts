import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import Icons from "unplugin-icons/vite";

/** Turns an Iconify SVG into a Svelte 5 component that takes `size` (px) plus any other SVG attribute. */
function svelteIcon(svg: string): string {
  const body = svg
    .replace(/ width="[^"]*"/, "")
    .replace(/ height="[^"]*"/, "")
    .replace("<svg ", '<svg {...rest} width={size} height={size} aria-hidden="true" ');
  return `<script>let { size = 16, ...rest } = $props();</script>\n${body}\n`;
}

// The frontend lives in ui/; Tauri loads the dev server in `tauri dev` and dist/ in release builds.
export default defineConfig({
  root: "ui",
  plugins: [
    svelte(),
    // `~icons/solar/<name>` imports become components at build time; only icons the UI uses are bundled.
    Icons({ compiler: { extension: "svelte", compiler: svelteIcon } }),
  ],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { outDir: "../dist", emptyOutDir: true, target: "es2022" },
});
