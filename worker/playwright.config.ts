import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "../e2e",
  // Per-platform baselines for visual.spec.ts (font rendering differs by OS).
  snapshotPathTemplate: "{testDir}/__screenshots__/{arg}-{platform}{ext}",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  // One wrangler dev isolate serves every page. Parallel pages starved it and
  // readiness waits timed out locally, even at 2 workers (2026-09-29), so
  // local runs match CI.
  workers: 1,
  reporter: "html",
  use: {
    baseURL: process.env.BASE_URL || "http://localhost:3001",
    trace: "on-first-retry",
    screenshot: "only-on-failure",
  },
});
