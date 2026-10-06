import { test, expect } from "@playwright/test";

test.describe("Landing page", () => {
  test("loads and shows hero content", async ({ page }) => {
    await page.goto("/");
    await expect(page).toHaveTitle(/BeThere/);
    // One headline, one primary CTA (.plans/038 P1-1; the hero is .plans/043 L2).
    await expect(page.locator(".lp-hero h1")).toBeVisible();
    await expect(page.locator(".lp-hero .lp-btn-primary")).toHaveCount(1);
  });

  test("navigates to login page", async ({ page }) => {
    await page.goto("/");
    const loginLink = page.locator('a[href="/login"]');
    if (await loginLink.isVisible()) {
      await loginLink.click();
      await expect(page).toHaveURL(/\/login/);
    }
  });

  // A real 404 from the worker, not the SPA shell answering 200 (release 2,
  // worker/src/crawl.rs route_kind): crawlers must not index made-up paths.
  test("shows 404 for unknown routes", async ({ page }) => {
    const resp = await page.goto("/this-page-does-not-exist");
    expect(resp?.status()).toBe(404);
    await expect(page.locator("h1")).toContainText("Not found");
  });
});
