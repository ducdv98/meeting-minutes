// Captures website screenshots of the real UI running on demo data.
// Usage: pnpm screenshots            website screenshots, 1200 × 800 at 2×
//        pnpm screenshots --gallery  Product Hunt gallery images, 1270 × 760
import { createServer } from "vite";
import { chromium } from "playwright";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { mkdir, realpath } from "node:fs/promises";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
const nodeModules = await realpath(join(root, "node_modules"));
const gallery = process.argv.includes("--gallery");
// Gallery images are launch material, not website assets.
const out = join(root, gallery ? ".scratch/v1-launch/gallery" : "website/src/assets/screens");
const viewport = gallery ? { width: 1270, height: 760 } : { width: 1200, height: 800 };
const deviceScaleFactor = gallery ? 1 : 2;
const DEMO = "Website launch sync";

const server = await createServer({ root, configFile: join(root, "vite.config.ts"), server: { port: 0, fs: { allow: [root, nodeModules] } }, logLevel: "error" });
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
  async "firstrun-model"(page) {
    await page.getByRole("button", { name: "Help" }).click();
    await page.getByRole("button", { name: "Open the setup guide" }).click();
    await page.getByRole("button", { name: "I understand" }).click();
    await page.getByRole("heading", { name: "Set up a local model" }).waitFor();
  },
};

await mkdir(out, { recursive: true });
const browser = await chromium.launch();
const errors = [];
for (const theme of ["light", "dark"]) {
  for (const [name, prepare] of Object.entries(shots)) {
    const page = await browser.newPage({ viewport, deviceScaleFactor, colorScheme: theme, reducedMotion: "reduce" });
    page.on("console", (message) => { if (message.type() === "warning" || message.type() === "error") errors.push(`${name}: ${message.text()}`); });
    await page.route("http://asset.localhost/**", (route) => route.fulfill({ status: 204 }));
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
