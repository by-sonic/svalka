import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// `vitest/config` re-exports Vite's defineConfig plus the `test` field.
export default defineConfig({
  plugins: [svelte()],
  // Tauri wants a fixed dev port and an untouched console.
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  test: {
    environment: "node",
    include: ["src/**/*.{test,spec}.{ts,js}"],
  },
});
