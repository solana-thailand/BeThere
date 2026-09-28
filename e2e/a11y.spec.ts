// axe-core gate (.plans/038 P0-3): no serious or critical WCAG 2.1 A/AA
// violation on any listed page, at either viewport, unless it is in
// a11y-allowlist.json. Entries are per element ("<rule> <selector>"), so an
// allowed element cannot hide a new one on the same page. The allowlist only
// shrinks: an entry that no longer occurs fails the run until it is deleted.
import { test, expect } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { PAGES, VIEWPORTS, openPage } from "./pages";

test.use({ serviceWorkers: "block" });

const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"];
const GATED = new Set(["serious", "critical"]);

/// { "<page name>": { "<axe rule id> <selector substring>": "why, and the issue" } }
const ALLOWLIST: Record<string, Record<string, string>> = JSON.parse(
  readFileSync(join(__dirname, "a11y-allowlist.json"), "utf8"),
);

const splitEntry = (entry: string) => {
  const space = entry.indexOf(" ");
  return { rule: entry.slice(0, space), selector: entry.slice(space + 1) };
};

for (const target of PAGES) {
  test(`a11y: ${target.name}`, async ({ page, context, baseURL }) => {
    const allowed = Object.keys(ALLOWLIST[target.name] ?? {}).map(entry => ({ entry, ...splitEntry(entry) }));
    const seen = new Set<string>();
    const unexpected: string[] = [];
    for (const viewport of VIEWPORTS) {
      await page.setViewportSize(viewport);
      await openPage(page, context, target, baseURL!);
      const results = await new AxeBuilder({ page }).withTags(TAGS).analyze();
      for (const violation of results.violations) {
        if (!GATED.has(violation.impact ?? "")) continue;
        for (const node of violation.nodes) {
          const where = node.target.join(" ");
          // axe reports the shortest unique selector (often just ".btn"), so
          // a ".class" entry also matches the element's own class list.
          const classes = (node.html.match(/class="([^"]*)"/)?.[1] ?? "").split(/\s+/);
          const hit = allowed.find(a => a.rule === violation.id
            && (where.includes(a.selector) || (a.selector.startsWith(".") && classes.includes(a.selector.slice(1)))));
          if (hit) {
            seen.add(hit.entry);
            continue;
          }
          unexpected.push(`[${viewport.name}] ${violation.impact} ${violation.id}: ${where} — ${node.html.slice(0, 100)}`);
        }
      }
    }
    expect(unexpected, "new serious/critical axe violations").toEqual([]);
    const stale = allowed.map(a => a.entry).filter(entry => !seen.has(entry));
    expect(stale, "allowlist entries that no longer occur: delete them").toEqual([]);
  });
}

// The gate must be able to fail. Inject two known critical violations and
// require axe to report both; if this passes vacuously, the gate is blind.
test("a11y gate self-test: injected violations are caught", async ({ page, context, baseURL }) => {
  await openPage(page, context, PAGES[0], baseURL!);
  await page.evaluate(() => {
    const probe = document.createElement("div");
    probe.id = "a11y-self-test";
    probe.innerHTML = '<button></button><img src="/favicon.ico">';
    document.body.appendChild(probe);
  });
  const results = await new AxeBuilder({ page }).include("#a11y-self-test").withTags(TAGS).analyze();
  const ids = results.violations.filter(v => GATED.has(v.impact ?? "")).map(v => v.id).sort();
  expect(ids).toEqual(["button-name", "image-alt"]);
});
