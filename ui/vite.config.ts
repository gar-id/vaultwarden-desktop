import { defineConfig, type Plugin } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";

// Vite emits crossorigin="" on the entry <script>/<link> tags by default,
// which assumes the page is served over real HTTP. Tauri's production build
// serves index.html from its own tauri://localhost custom protocol, and
// WebKit cancels the asset load when crossorigin is present but the
// response doesn't carry CORS headers for it — this produces a blank
// window with "Operation was cancelled" and no Rust-side log output at all.
// There's no Vite build option for this; Rollup always adds it when
// `build.target` triggers module preload. Strip it post-build instead.
function stripCrossorigin(): Plugin {
  return {
    name: "strip-crossorigin-for-tauri",
    transformIndexHtml(html) {
      return html.replace(/ crossorigin(="[^"]*")?/g, "");
    },
  };
}

export default defineConfig({
  plugins: [tailwindcss(), svelte(), stripCrossorigin()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "chrome105",
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
