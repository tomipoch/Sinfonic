import path from "node:path";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  test: {
    environment: "happy-dom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    css: false,
    coverage: {
      provider: "v8",
      reporter: ["text", "text-summary", "html"],
      reportsDirectory: "./coverage",
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "src/**/*.test.{ts,tsx}",
        "src/test/**",
        "src/main.tsx",
        "src/settings/main.tsx",
        "src/**/*.d.ts",
      ],
      // P3 thresholds — bumped after centralising helpers into lib/ +
      // stores/. Each subsequent phase should ratchet these up as
      // coverage expands.
      thresholds: {
        "./src/lib/**": {
          lines: 60,
          functions: 30,
          branches: 55,
          statements: 50,
        },
        "./src/hooks/**": {
          lines: 25,
          functions: 29,
          branches: 20,
          statements: 25,
        },
        "./src/stores/**": {
          lines: 42,
          functions: 39,
          branches: 35,
          statements: 43,
        },
      },
    },
  },
});
