// Visual snapshots (.plans/038 P0-3): every listed page at 390x844 and
// 1440x900 against a committed baseline.
//
// Baselines are per platform (font rendering differs), and CI runs Linux.
// Update after an intentional change, on the platform whose baseline changed:
//   cd worker && pnpm exec playwright test visual --update-snapshots
// CI runs with --update-snapshots=missing and uploads what it wrote as the
// `visual-baselines` artifact; commit those PNGs to add a Linux baseline.
import path from "node:path";
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
        // Content that is correct but not fixed: the clock, camera state, the
        // build line (commit hash; absent from CI's plain trunk build) and the
        // landing's live stats (measured-at time; figures that count up only
        // once the strip is seen, so a full-page capture can catch any value).
        // The scanner <video> fills the viewport, so it is hidden by
        // screenshot.css rather than masked (a mask covers the UI above it).
        mask: [page.locator(".dashboard-last-updated, canvas, .lp-version, .lp-livebar, .lp-done")],
        stylePath: path.join(__dirname, "screenshot.css"),
        maxDiffPixelRatio: 0.01,
      });
    });
  }
}

// Inline ticket on the signed-in landing (.plans/038 P2-a), both states.
test("visual: landing signed in, inline ticket collapsed and expanded", async ({ page, context, baseURL }) => {
  await page.setViewportSize(VIEWPORTS[0]);
  await openPage(page, context, { name: "landing-authed", path: "/", authed: true, ready: ".landing-reg-qr-toggle" }, baseURL!);
  const section = page.locator(".landing-reg-section");
  await expect(section).toHaveScreenshot("landing-ticket-collapsed-mobile.png", { animations: "disabled" });
  await page.locator(".landing-reg-qr-toggle").click();
  await expect(page.locator(".landing-reg-qr-svg")).toBeVisible();
  await expect(section).toHaveScreenshot("landing-ticket-expanded-mobile.png", { animations: "disabled" });
});
