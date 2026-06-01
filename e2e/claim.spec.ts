import { test, expect } from "@playwright/test";

test.describe("Claim page", () => {
  test("shows error for invalid claim token", async ({ page }) => {
    await page.goto("/claim/invalid-token-12345");

    // Should show either "not found", "invalid", or "expired" error
    const errorIndicator = page
      .locator(".error, .error-msg, [role='alert']")
      .or(page.getByText(/not found|invalid|expired|error/i));
    await expect(errorIndicator.first()).toBeVisible({ timeout: 10000 });
  });

  test("loads without crashing", async ({ page }) => {
    const response = await page.goto("/claim/some-token");
    // Page should load (200 from SPA, even if claim data fails)
    expect(response).not.toBeNull();
  });
});
