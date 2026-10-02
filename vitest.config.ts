import { defineConfig } from "vitest/config";

// Frontend unit tests (`npm test`). Pure logic only — no DOM; components are
// covered by the e2e smoke tests instead.
export default defineConfig({
  test: {
    include: ["src/**/*.test.ts"],
    environment: "node",
  },
});
