import { test, expect } from "@playwright/test";

test.describe("Auth-protected routes", () => {
  test("admin redirects to login when not authenticated", async ({ page }) => {
    await page.goto("/admin");
    // Should end up on login page (protected route redirects)
    await expect(page).toHaveURL(/\/login/, { timeout: 5000 });
  });

  test("staff redirects to login when not authenticated", async ({ page }) => {
    await page.goto("/staff");
    await expect(page).toHaveURL(/\/login/, { timeout: 5000 });
  });
});
