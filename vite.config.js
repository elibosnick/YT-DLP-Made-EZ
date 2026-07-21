import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],

  // Tauri expects a fixed port and fails if it is unavailable, rather than silently
  // moving to another one and leaving the webview pointed at nothing.
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Rust rebuilds are driven by cargo; watching src-tauri from Vite just causes
      // redundant reloads mid-compile.
      ignored: ["**/src-tauri/**"],
    },
  },

  // Tauri's webviews are current Chromium/WebKit, so there is no reason to ship
  // legacy transpilation or polyfills.
  build: {
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },

  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test-setup.js"],
  },
});
