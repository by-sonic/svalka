import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// `vitest/config` re-exports Vite's defineConfig plus the `test` field.
export default defineConfig({
  plugins: [svelte()],
  // Tauri wants a fixed dev port and an untouched console.
  clearScreen: false,
  server: {
    // 5174, not Vite's default 5173 — the Ruscord project owns 5173, so a
    // fixed dedicated port lets both dev servers run at once. strictPort keeps
    // it honest: if 5174 is also taken it fails loudly instead of silently
    // picking another port and desyncing Tauri's devUrl.
    port: 5174,
    strictPort: true,
  },
  test: {
    environment: "node",
    include: ["src/**/*.{test,spec}.{ts,js}"],
  },
});
