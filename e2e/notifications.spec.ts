import { test, expect, type Page, type Route } from "@playwright/test";

test.use({ serviceWorkers: "block" });

const event = {
  id: "notification-test", name: "Notification test event", slug: "notification-test",
  status: "active", event_format: "in_person", event_start_ms: 1_900_000_000_000,
  event_end_ms: 1_900_003_600_000, organizer_emails: ["organizer@example.com"],
};
const delivery = (id: number, status: string, name: string) => ({
  id, kind: "registration", status, attempts: 1,
  error_code: status === "failed" ? "E_SENDER_NOT_VERIFIED" : null,
  recipient_name: name, recipient_email: `${name.toLowerCase()}@example.com`,
});
const inboxItem = (id: number, title: string, read = false) => ({
  id, title, body: `${title} body`, event_name: "Builder night",
  action_url: "/ticket/a?event_id=event-a", action_label: "View ticket",
  read_at: read ? 1 : null,
});
const json = (route: Route, data: unknown) => route.fulfill({
  contentType: "application/json", headers: { "cache-control": "no-store" },
  body: JSON.stringify({ success: true, data }),
});

async function mockApp(page: Page, notificationRoute: (route: Route) => Promise<void>) {
  await page.route("**/*", async (route) => {
    const url = new URL(route.request().url());
    // Local assets and mocked APIs only: tests cannot send email or contact production.
    if (!["127.0.0.1", "localhost"].includes(url.hostname)) return route.abort();
    if (!url.pathname.startsWith("/api/")) return route.continue();
    if (url.pathname === "/api/auth/me") {
      return json(route, { email: "organizer@example.com", sub: "google-id", role: "super_admin" });
    }
    if (url.pathname === "/api/events") return json(route, { events: [event] });
    if (url.pathname === "/api/events/notification-test") return json(route, { event });
    if (url.pathname.startsWith("/api/events/notification-test/notifications")) {
      return notificationRoute(route);
    }
    if (url.pathname.startsWith("/api/api/")) return route.fulfill({ status: 404 });
    return json(route, { attendees: [], stats: {}, events: [], items: [] });
  });
  await page.goto("/admin");
  await expect(page.getByRole("button", { name: "Notifications", exact: true })).toBeVisible();
}

async function mockAttendeeInbox(page: Page, notificationRoute: (route: Route) => Promise<void>) {
  await page.route("**/*", async (route) => {
    const url = new URL(route.request().url());
    if (!["127.0.0.1", "localhost"].includes(url.hostname)) return route.abort();
    if (!url.pathname.startsWith("/api/")) return route.continue();
    if (url.pathname === "/api/auth/me") {
      return json(route, {
        email: "attendee@example.com", sub: "google-id", role: "attendee",
        email_verified: true,
      });
    }
    if (url.pathname === "/api/my-registrations") return json(route, []);
    if (url.pathname.startsWith("/api/my-notifications")) return notificationRoute(route);
    return json(route, { events: [], items: [] });
  });
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Notifications" })).toBeVisible();
}

test("delivery history paginates and retries only failed messages", async ({ page }) => {
  let retried = false;
  await mockApp(page, async route => {
    const url = new URL(route.request().url());
    if (url.pathname.endsWith("/retry")) {
      expect(route.request().method()).toBe("POST");
      expect(route.request().postDataJSON()).toEqual({ notification_id: 30 });
      retried = true;
      return json(route, { queued: true });
    }
    if (url.searchParams.has("before")) {
      expect(url.searchParams.get("before")).toBe("29");
      return json(route, { items: [delivery(28, "accepted", "Older")], next_before: null });
    }
    return json(route, {
      items: [delivery(30, retried ? "pending" : "failed", "Alice"), delivery(29, "uncertain", "Bob")],
      next_before: 29,
    });
  });
  await page.getByRole("button", { name: "Notifications", exact: true }).click();
  const panel = page.locator(".notification-panel");
  await expect(panel.getByRole("button", { name: "Retry", exact: true })).toHaveCount(1);
  await panel.getByRole("button", { name: "Retry", exact: true }).click();
  await expect(panel.getByText("Queued", { exact: true })).toBeVisible();
  await expect(panel.getByRole("button", { name: "Retry", exact: true })).toHaveCount(0);
  await panel.getByRole("button", { name: "Older messages" }).click();
  await expect(panel.getByText("Older", { exact: true })).toBeVisible();
  await expect(panel.getByRole("button", { name: "Older messages" })).toBeDisabled();
});

test("late response from a closed panel cannot replace newer history", async ({ page }) => {
  let calls = 0;
  let release: (() => void) | undefined;
  const blocked = new Promise<void>(resolve => { release = resolve; });
  await mockApp(page, async route => {
    calls += 1;
    const first = calls === 1;
    if (first) await blocked;
    return json(route, { items: [delivery(first ? 1 : 2, "accepted", first ? "Stale" : "Fresh")], next_before: null });
  });
  const toggle = page.getByRole("button", { name: "Notifications", exact: true });
  await toggle.click();
  await expect.poll(() => calls).toBe(1);
  await toggle.click();
  await toggle.click();
  await expect(page.locator(".notification-panel").getByText("Fresh", { exact: true })).toBeVisible();
  const oldResponse = page.waitForResponse(response => response.url().endsWith("/notifications"));
  release!();
  await oldResponse;
  // A rendering turn after completion makes the stale-write assertion observable.
  await page.evaluate(() => new Promise(requestAnimationFrame));
  await expect(page.locator(".notification-panel").getByText("Fresh", { exact: true })).toBeVisible();
  await expect(page.locator(".notification-panel").getByText("Stale", { exact: true })).toHaveCount(0);
});

test("mobile delivery table stays within its panel", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await mockApp(page, route => json(route, {
    items: [delivery(1, "failed", "BuilderWithALongDisplayName"), delivery(2, "uncertain", "Bob")], next_before: null,
  }));
  await page.getByRole("button", { name: "Notifications", exact: true }).click();
  const panel = page.locator(".notification-panel");
  await expect(panel.getByText("Needs review", { exact: true })).toBeVisible();
  const bounds = await panel.locator(".notification-table-scroll").boundingBox();
  expect(bounds).not.toBeNull();
  expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(391);
  await expect(panel.getByText("Swipe across the table to see status and retry actions.")).toBeVisible();
  const tableWidth = await panel.locator("table").evaluate(el => el.getBoundingClientRect().width);
  expect(tableWidth).toBeGreaterThan(600);
  await panel.screenshot({ path: testInfo.outputPath("notifications-mobile.png") });
});

test("attendee inbox paginates and updates unread state", async ({ page }) => {
  const readIds: number[] = [];
  await mockAttendeeInbox(page, async route => {
    const url = new URL(route.request().url());
    const read = url.pathname.match(/\/my-notifications\/(\d+)\/read$/);
    if (read) {
      readIds.push(Number(read[1]));
      return json(route, { read: true });
    }
    if (url.searchParams.get("before") === "49") {
      return json(route, { items: [inboxItem(48, "Older")], unread_count: 2, next_before: null });
    }
    return json(route, {
      items: [inboxItem(50, "Latest"), inboxItem(49, "Previous")],
      unread_count: 2, next_before: 49,
    });
  });

  const inbox = page.locator(".attendee-inbox");
  await expect(inbox.getByText("2 unread", { exact: true })).toBeVisible();
  await inbox.getByRole("button", { name: "Mark read" }).first().click();
  await expect.poll(() => readIds).toEqual([50]);
  await expect(inbox.getByText("1 unread", { exact: true })).toBeVisible();
  await inbox.getByRole("button", { name: "Older notifications" }).click();
  await expect(inbox.getByText("Older", { exact: true })).toBeVisible();
  await expect(inbox.getByRole("button", { name: "Older notifications" })).toHaveCount(0);
});

test("attendee inbox surfaces API failures", async ({ page }) => {
  await mockAttendeeInbox(page, route => route.fulfill({
    status: 503,
    contentType: "application/json",
    body: JSON.stringify({ success: false, error: "Inbox temporarily unavailable" }),
  }));
  await expect(page.locator(".attendee-inbox").getByRole("alert")).toContainText("Inbox temporarily unavailable");
});
