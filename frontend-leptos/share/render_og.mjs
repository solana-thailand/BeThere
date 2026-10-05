// Renders share/og-card.html to share/og-image.png at 1200x630 (plan 042, 0.6).
// Usage, from frontend-leptos/:
//   FONTS_DIR=~/solana-thailand-devrel-helper/assets/fonts-active \
//   PUPPETEER=~/.npm/_npx/<hash>/node_modules/puppeteer-core \
//   node share/render_og.mjs
// The fonts (Inter, Anuphan; both OFL) are only needed to render, not shipped.
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const here = dirname(fileURLToPath(import.meta.url));
const fonts_dir = process.env.FONTS_DIR;
const puppeteer_dir = process.env.PUPPETEER;
for (const [name, path] of [["FONTS_DIR", fonts_dir], ["PUPPETEER", puppeteer_dir]]) {
    if (!path || !existsSync(path)) {
        console.error(`${name} is not set or does not exist: ${path}`);
        process.exit(2);
    }
}
for (const font of ["InterVariable.ttf", "Anuphan-Variable.ttf"]) {
    if (!existsSync(join(fonts_dir, font))) {
        console.error(`missing font: ${join(fonts_dir, font)}`);
        process.exit(2);
    }
}

const puppeteer = createRequire(import.meta.url)(puppeteer_dir);
const html = readFileSync(join(here, "og-card.html"), "utf8").replaceAll("FONTS_DIR", `file://${fonts_dir}`);
const tmp = join(here, ".og-card.render.html");
writeFileSync(tmp, html);

const browser = await puppeteer.launch({
    executablePath: "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    args: ["--allow-file-access-from-files"],
});
try {
    const page = await browser.newPage();
    await page.setViewport({ width: 1200, height: 630, deviceScaleFactor: 1 });
    await page.goto(`file://${tmp}`, { waitUntil: "load" });
    await page.evaluate(() => document.fonts.ready);
    // A silent fallback to a system font would still "work"; refuse it.
    const loaded = await page.evaluate(() =>
        ["Inter", "Anuphan"].filter((f) => !document.fonts.check(`700 40px ${f}`)),
    );
    if (loaded.length) throw new Error(`fonts not loaded: ${loaded.join(", ")}`);
    await page.screenshot({ path: join(here, "og-image.png"), type: "png" });
    console.log(`wrote ${join(here, "og-image.png")}`);
} finally {
    await browser.close();
    (await import("node:fs")).rmSync(tmp, { force: true });
}
