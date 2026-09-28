// Visual snapshots (.plans/038 P0-3): every listed page at 390x844 and
// 1440x900 against a committed baseline.
//
// Baselines are per platform (font rendering differs), and CI runs Linux.
// Update after an intentional change, on the platform whose baseline changed:
//   cd worker && pnpm exec playwright test visual --update-snapshots
// CI runs with --update-snapshots=missing and uploads what it wrote as the
// `visual-baselines` artifact; commit those PNGs to add a Linux baseline.
import { test, expect } from "@playwright/test";
import { PAGES, VIEWPORTS, openPage } from "./pages";

test.use({ serviceWorkers: "block" });

for (const target of PAGES) {
  for (const viewport of VIEWPORTS) {
    test(`visual: ${target.name} @ ${viewport.name}`, async ({ page, context, baseURL }) => {
      await page.setViewportSize(viewport);
      await openPage(page, context, target, baseURL!);
      await expect(page).toHaveScreenshot(`${target.name}-${viewport.name}.png`, {
        fullPage: true,
        animations: "disabled",
        caret: "hide",
        // Content that is correct but not fixed: the clock and camera state.
        mask: [page.locator(".dashboard-last-updated, video, canvas")],
        maxDiffPixelRatio: 0.01,
      });
    });
  }
}
