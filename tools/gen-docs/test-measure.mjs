// Measure key element positions on the rendered page to detect
// layout regressions vs master.
import { chromium } from "playwright";
import { fileURLToPath } from "url";
import { dirname, resolve } from "path";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..", "..");
const pageUrl = "file://" + resolve(repoRoot, "gh-pages", "index.html");

const browser = await chromium.launch({
  executablePath: "/opt/pw-browsers/chromium-1194/chrome-linux/chrome",
  headless: true,
});

for (const w of [600, 800, 1280]) {
  const ctx = await browser.newContext({ viewport: { width: w, height: 900 } });
  const page = await ctx.newPage();
  await page.goto(pageUrl);
  await page.waitForTimeout(100);
  const info = await page.evaluate(() => {
    const body = document.body;
    const h1 = document.getElementById("catalogue");
    const drawer = document.querySelector(".nav-drawer");
    const nav = document.querySelector(".nav-sidebar");
    const r = (el) => el ? el.getBoundingClientRect().toJSON() : null;
    return {
      bodyMarginTop: window.getComputedStyle(body).marginTop,
      htmlPaddingLeft: window.getComputedStyle(document.documentElement).paddingLeft,
      drawerDisplay: drawer ? window.getComputedStyle(drawer).display : null,
      drawerRect: r(drawer),
      navDisplay: nav ? window.getComputedStyle(nav).display : null,
      navPosition: nav ? window.getComputedStyle(nav).position : null,
      h1Rect: r(h1),
    };
  });
  console.log(`viewport ${w}x900:`, JSON.stringify(info, null, 2));
  await ctx.close();
}
await browser.close();
