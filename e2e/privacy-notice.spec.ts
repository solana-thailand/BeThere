// First-visit privacy notice (.plans/038 P2-d): shown once on attendee pages,
// links to /privacy, and stays dismissed across reloads.
import { test, expect } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

test.use({ serviceWorkers: "block", viewport: { width: 390, height: 844 } });

test("privacy notice: first visit, link, dismissal persists", async ({ page }) => {
  await page.route(/static\.cloudflareinsights\.com/, route => route.abort());
  await page.goto("/", { waitUntil: "load" });
  const notice = page.locator(".privacy-notice");
  await expect(notice).toBeVisible();
  await expect(notice.locator('a[href="/privacy"]')).toBeVisible();
  const results = await new AxeBuilder({ page }).include(".privacy-notice").analyze();
  expect(results.violations.map(v => v.id)).toEqual([]);
  await expect(notice).toHaveScreenshot("privacy-notice-mobile.png");

  await notice.getByRole("button").click();
  await expect(notice).toHaveCount(0);
  await page.reload({ waitUntil: "load" });
  await page.waitForTimeout(500);
  await expect(page.locator(".privacy-notice")).toHaveCount(0);
});
