// Headless-Chromium render harness for the generated catalogue page.
// Loads gh-pages/index.html at a target viewport size, scrolls to
// requested positions, and writes PNG screenshots so changes to the
// nav-drawer / hamburger reveal can be eyeballed without a real
// browser in the loop.
//
// Setup (once per fresh container):
//   - `just gen-docs` to refresh `gh-pages/index.html`
//   - from this directory, symlink the global Playwright install:
//       ln -sf /opt/node22/lib/node_modules node_modules
//
// Run:
//   cd tools/gen-docs && node test-render.mjs
// Writes screenshots under tools/gen-docs/test-screenshots/ (which
// `.gitignore` already excludes).
//
// Caveats: Chromium only — for cross-engine concerns (Firefox /
// Safari support of a CSS feature), pair this with a caniuse check.

import { chromium } from "playwright";
import { fileURLToPath } from "url";
import { dirname, resolve } from "path";
import { mkdirSync } from "fs";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..", "..");
const pageUrl = "file://" + resolve(repoRoot, "gh-pages", "index.html");
const outDir = resolve(here, "test-screenshots");
mkdirSync(outDir, { recursive: true });

// Use the pre-installed Chromium under /opt/pw-browsers.
const browser = await chromium.launch({
  executablePath: "/opt/pw-browsers/chromium-1194/chrome-linux/chrome",
  headless: true,
});

const cases = [
  // [label, viewport-width, viewport-height, scroll-y]
  ["narrow-top", 600, 900, 0],
  ["narrow-after-index", 600, 900, 1200],
  ["narrow-deep", 600, 900, 4000],
  ["tablet-portrait-top", 800, 1200, 0],
  ["tablet-portrait-after-index", 800, 1200, 1200],
  ["tablet-landscape", 1280, 800, 0],
  ["desktop", 1440, 900, 0],
];

let failed = false;
for (const [label, w, h, y] of cases) {
  const ctx = await browser.newContext({ viewport: { width: w, height: h } });
  const page = await ctx.newPage();
  await page.goto(pageUrl);
  await page.evaluate((y) => window.scrollTo(0, y), y);
  // IntersectionObserver fires asynchronously; give it one rAF + a
  // little slack before screenshotting.
  await page.waitForTimeout(150);

  const hamburgerState = await page.evaluate(() => {
    const el = document.querySelector(".nav-toggle");
    if (!el) return { present: false };
    const style = window.getComputedStyle(el);
    return {
      present: true,
      display: style.display,
      opacity: style.opacity,
      visibility: style.visibility,
      hasVisibleClass: el.classList.contains("nav-toggle-visible"),
      rect: el.getBoundingClientRect().toJSON(),
    };
  });

  const sidebarState = await page.evaluate(() => {
    const el = document.querySelector(".nav-sidebar");
    if (!el) return { present: false };
    const style = window.getComputedStyle(el);
    return {
      present: true,
      display: style.display,
      visibility: style.visibility,
      rect: el.getBoundingClientRect().toJSON(),
    };
  });

  const out = resolve(outDir, `${label}.png`);
  await page.screenshot({ path: out, fullPage: false });
  console.log(`[${label}] ${w}x${h} scrollY=${y}  ->  ${out}`);
  console.log("  hamburger:", JSON.stringify(hamburgerState));
  console.log("  sidebar:  ", JSON.stringify(sidebarState));
  await ctx.close();
}

await browser.close();
if (failed) process.exit(1);
