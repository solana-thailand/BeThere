// Shared page list for visual.spec.ts and a11y.spec.ts (.plans/038 P0-3).
//
// The authed pages need the worker in DEV_MODE with the fixture loaded
// (e2e/fixtures/seed.sql + POST /api/events/reseed-kv); CI's e2e job does
// both. Override the fixture ids with E2E_* env vars to point at other data.
import type { BrowserContext, Page } from "@playwright/test";

export const EVENT_ID = process.env.E2E_EVENT_ID ?? "e2e-event";
export const EVENT_SLUG = process.env.E2E_EVENT_SLUG ?? "e2e-builders-night";
export const ATTENDEE_ID = process.env.E2E_ATTENDEE_ID ?? "e2e-att-01";
export const EVENT_NAME = process.env.E2E_EVENT_NAME ?? "E2E Builders Night";
export const CLAIM_TOKEN = process.env.E2E_CLAIM_TOKEN ?? "0190e2e0-0000-7000-8000-000000000001";

export interface AppPage {
  name: string;
  path: string;
  authed: boolean;
  /// Visible once the page's data has landed. Snapshots and axe run after
  /// it, so a slow API cannot shrink what they see.
  ready: string;
}

const EVENT_LOADED = `text=${EVENT_NAME}`;

export const PAGES: AppPage[] = [
  { name: "landing", path: "/", authed: false, ready: EVENT_LOADED },
  { name: "discover", path: "/discover", authed: false, ready: EVENT_LOADED },
  { name: "event", path: `/e/${EVENT_SLUG}`, authed: false, ready: EVENT_LOADED },
  { name: "privacy", path: "/privacy", authed: false, ready: ".pe-section-title" },
  { name: "admin", path: "/admin", authed: true, ready: EVENT_LOADED },
  // The scanner defaults to whichever active event it picks, so wait for its
  // event picker rather than a name.
  { name: "staff", path: "/staff", authed: true, ready: ".scanner-event-label" },
  { name: "ticket", path: `/ticket/${ATTENDEE_ID}?event_id=${EVENT_ID}`, authed: true, ready: EVENT_LOADED },
  { name: "claim", path: `/claim/${CLAIM_TOKEN}`, authed: false, ready: EVENT_LOADED },
];

export const VIEWPORTS = [
  { name: "mobile", width: 390, height: 844 },
  { name: "desktop", width: 1440, height: 900 },
] as const;

/// Browser clock for every page: two weeks before the fixture event, so
/// countdowns and "upcoming" copy render the same on every run.
const FIXED_NOW = new Date("2030-01-01T03:00:00Z");

/// Open `target` the way a visitor would, with the dev-token session when the
/// page needs one. Third-party beacons are dropped so they cannot flake a run.
export async function openPage(page: Page, context: BrowserContext, target: AppPage, baseURL: string) {
  await page.route(/static\.cloudflareinsights\.com/, route => route.abort());
  await page.clock.setFixedTime(FIXED_NOW);
  if (target.authed) {
    await context.addCookies([{ name: "event_checkin_token", value: "dev-token", url: baseURL }]);
    await page.addInitScript(() => localStorage.setItem("event_checkin_token", "dev-token"));
  }
  // Not "networkidle": the ticket and staff pages poll, so they never idle.
  await page.goto(target.path, { waitUntil: "load" });
  await page.evaluate(() => document.fonts.ready);
  await page.locator(target.ready).first().waitFor({ state: "visible" });
  // Sibling requests (badges, counts) that land just after the main one.
  await page.waitForTimeout(750);
}
