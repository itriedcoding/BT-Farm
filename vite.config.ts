import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed port and does not want Vite clearing the terminal.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: true,
    watch: { ignored: ["**/src-tauri/**", "**/crates/**"] },
  },
  build: {
    target: "chrome105",
    sourcemap: false,
    outDir: "dist",
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
