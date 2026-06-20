import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Vite config tuned for Tauri: fixed dev port, no clearing the terminal so
// Rust logs stay visible, and a build target matching the WebViews we support.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    // WebView2 (Win), WKWebView (macOS), WebKitGTK (Linux) all support es2021.
    target: "es2021",
    outDir: "dist",
    sourcemap: true,
  },
});
