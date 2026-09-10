import { defineConfig } from "vite";

// Tauri expects a fixed port and no clearing of the screen
export default defineConfig({
  clearScreen: false,
  server: {
    port: 5183,
    strictPort: true,
    watch: {
      // never let Vite watch the Rust build output — the compiler locks files
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    target: "esnext",
    outDir: "dist",
    emptyOutDir: true,
  },
});
