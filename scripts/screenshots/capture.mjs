// Captures website screenshots of the real UI running on demo data.
// Usage: pnpm screenshots
import { createServer } from "vite";
import { chromium } from "playwright";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
const out = join(root, "website/src/assets/screens");
const DEMO = "Website launch sync";

const server = await createServer({ root, configFile: join(root, "vite.config.ts"), server: { port: 0 }, logLevel: "error" });
await server.listen();
const url = `${server.resolvedUrls.local[0]}scripts/screenshots/`;

async function settle(page) {
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(400);
}

async function openDemo(page) {
  await page.getByRole("navigation", { name: "Meetings" }).getByRole("button", { name: DEMO, exact: true }).click();
  await page.getByRole("heading", { name: DEMO }).waitFor();
  await page.getByText("Revision 2 · large-v3-turbo-q5_0").waitFor();
}

const shots = {
  async meeting(page) {
    await openDemo(page);
  },
  async ask(page) {
    await openDemo(page);
    await page.getByRole("tab", { name: "Ask" }).click();
    await page.getByText("What did we decide about the Pro plan?").waitFor();
  },
  async transcript(page) {
    await openDemo(page);
    const transcript = page.getByRole("region", { name: "Transcript" });
    // Start the view on a Segment boundary so no text shows above the sticky header.
    await transcript.evaluate((element) => {
      const row = element.querySelector('[data-segment-start="43000"]');
      const header = element.querySelector(".sticky");
      element.scrollTop += row.getBoundingClientRect().top - element.getBoundingClientRect().top - header.offsetHeight;
    });
  },
  async search(page) {
    await page.getByRole("searchbox", { name: "Search transcripts" }).fill("pricing");
    await page.getByText("Customer interview — Northwind").last().waitFor();
    await page.mouse.move(0, 0);
  },
  async diagnostics(page) {
    await page.getByRole("button", { name: "Help" }).click();
    await page.getByLabel("Diagnostics preview").waitFor();
  },
  async library(page) {},
};

const browser = await chromium.launch();
const errors = [];
for (const theme of ["light", "dark"]) {
  for (const [name, prepare] of Object.entries(shots)) {
    const page = await browser.newPage({ viewport: { width: 1200, height: 800 }, deviceScaleFactor: 2, colorScheme: theme, reducedMotion: "reduce" });
    page.on("console", (message) => { if (message.type() === "warning" || message.type() === "error") errors.push(`${name}: ${message.text()}`); });
    await page.route("http://asset.localhost/**", (route) => route.fulfill({ status: 404 }));
    await page.goto(url);
    await page.getByRole("navigation", { name: "Meetings" }).getByRole("button", { name: DEMO, exact: true }).waitFor();
    await prepare(page);
    await settle(page);
    await page.screenshot({ path: join(out, `${name}-${theme}.png`) });
    await page.close();
    console.log(`${name}-${theme}.png`);
  }
}
await browser.close();
await server.close();
if (errors.length) console.warn(`Console messages:\n${[...new Set(errors)].join("\n")}`);
