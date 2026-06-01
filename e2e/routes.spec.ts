import { test, expect } from "@playwright/test";

test.describe("SPA routing", () => {
  test("privacy page loads", async ({ page }) => {
    await page.goto("/privacy");
    await expect(page.locator("body")).toBeVisible();
  });

  test("public event page handles missing slug", async ({ page }) => {
    const response = await page.goto("/e/nonexistent-event-slug");
    expect(response).not.toBeNull();
    // Should either show error or redirect
  });

  test("deposit page handles missing attendee", async ({ page }) => {
    const response = await page.goto("/deposit/nonexistent-id");
    expect(response).not.toBeNull();
  });
});
