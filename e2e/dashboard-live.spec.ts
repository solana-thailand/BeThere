import { test, expect, type Page, type Route } from "@playwright/test";

test.use({ serviceWorkers: "block" });

const json = (route: Route, data: unknown) => route.fulfill({
  contentType: "application/json",
  headers: { "cache-control": "no-store" },
  body: JSON.stringify({ success: true, data }),
});

const snapshot = (registered: number) => ({
  event: {
    id: "event-a", name: "Builder night", slug: "builder-night",
    in_person_capacity: 100, deposit_amount_usdc: 0, event_start_ms: 1_900_000_000_000,
  },
  totals: {
    registered, deposits_verified: 0, usdc_locked_total: 0,
    checked_in: 0, claims_minted: 0,
  },
  funnel: [], recent_activity: [], generated_at: "2026-09-10T00:00:00Z",
});

async function mockDashboard(page: Page, dashboardRoute: (route: Route) => Promise<void>) {
  await page.route("**/*", async route => {
    const url = new URL(route.request().url());
    if (!["127.0.0.1", "localhost"].includes(url.hostname)) return route.abort();
    if (!url.pathname.startsWith("/api/")) return route.continue();
    if (url.pathname === "/api/auth/me") {
      return json(route, { email: "staff@example.com", sub: "google-id", role: "super_admin" });
    }
    if (url.pathname === "/api/dashboard/live") return dashboardRoute(route);
    return json(route, {});
  });
}

test("newer dashboard refresh wins over a late response", async ({ page }) => {
  let calls = 0;
  let release: (() => void) | undefined;
  const delayed = new Promise<void>(resolve => { release = resolve; });
  await mockDashboard(page, async route => {
    calls += 1;
    if (calls === 2) await delayed;
    return json(route, snapshot(calls));
  });

  await page.goto("/dashboard/live?event_id=event-a");
  const registered = page.locator(".tile-blue .dashboard-tile-value");
  await expect(registered).toHaveText("1");
  await page.getByRole("button", { name: "↻ Refresh" }).click();
  await expect.poll(() => calls).toBe(2);
  await page.getByRole("button", { name: "↻ Refresh" }).click();
  await expect(registered).toHaveText("3");
  release!();
  await page.evaluate(() => new Promise(requestAnimationFrame));
  await expect(registered).toHaveText("3");
});

test("paused dashboard stops polling after unmount", async ({ page }) => {
  let calls = 0;
  await mockDashboard(page, route => {
    calls += 1;
    return json(route, snapshot(calls));
  });

  await page.goto("/dashboard/live?event_id=event-a");
  await expect.poll(() => calls).toBe(1);
  await page.getByRole("button", { name: "⏸ Pause" }).click();
  await page.goto("/privacy");
  await page.waitForTimeout(1_200);
  expect(calls).toBe(1);
});
