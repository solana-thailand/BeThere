import { test, expect } from "@playwright/test";

test.describe("Login page", () => {
  test("shows sign-in form", async ({ page }) => {
    await page.goto("/login");
    await expect(
      page.getByRole("button", { name: "Sign in with Google" }),
    ).toBeVisible();
  });

  test("has back to home link", async ({ page }) => {
    await page.goto("/login");
    await expect(page.locator('a[href="/"]')).toBeVisible();
  });

  test("shows powered by Solana badge", async ({ page }) => {
    await page.goto("/login");
    await expect(page.locator("text=Powered by Solana")).toBeVisible();
  });
});
