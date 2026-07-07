import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],

  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`:
  //
  // 1. prevent Vite from obscuring Rust compilation errors
  clearScreen: false,
  // 2. tauri expects a fixed port — fail if not available
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // tell Vite to rebuild when source files change (excluding Rust sources)
      ignored: ["**/src-tauri/**"],
    },
  },
}));
