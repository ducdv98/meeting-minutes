import test from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const run = promisify(execFile);
const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

async function* htmlFiles(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) yield* htmlFiles(path);
    else if (entry.name.endsWith('.html')) yield path;
  }
}

test('pricing shows Free, coming Pro and a way to support the project', async () => {
  const temp = await mkdtemp(join(tmpdir(), 'meeting-minutes-pricing-'));
  const output = join(temp, 'site');
  try {
    await run(process.execPath, ['website/build.mjs', '--draft', '--offline', '--output', output], { cwd: repo });
    const pricing = await readFile(join(output, 'pricing', 'index.html'), 'utf8');
    const home = await readFile(join(output, 'index.html'), 'utf8');
    const llms = await readFile(join(output, 'llms.txt'), 'utf8');

    for await (const path of htmlFiles(output)) {
      const page = await readFile(path, 'utf8');
      assert.doesNotMatch(page, /invite codes?|open source/i, `${relative(output, path)} uses current launch language`);
      for (const nav of ['Main navigation', 'Mobile navigation', 'Footer navigation']) {
        const content = page.match(new RegExp(`<nav\\b[^>]*aria-label="${nav}"[^>]*>([\\s\\S]*?)<\\/nav>`))?.[1];
        assert.ok(content, `${relative(output, path)} has ${nav}`);
        assert.match(content, /<a href="\/pricing\/"(?: aria-current="page")?>Pricing<\/a>/,
          `${relative(output, path)} links to pricing in ${nav}`);
      }
    }

    assert.match(home, /Is it free\?[\s\S]*?id="faq-free"[\s\S]*?Account[\s\S]*?<a href="\/pricing\/">/);
    assert.match(home, /id="faq-upload"[\s\S]*?Managed transcription[\s\S]*?Meeting Minutes service[\s\S]*?<a href="\/privacy\/">/);
    assert.match(llms, /^Pricing: https:\/\/meeting-minutes\.tech\/pricing\/$/m);
    assert.match(llms, /optional Account/i);
    assert.doesNotMatch(llms, /No account/i);
    assert.doesNotMatch(llms, /invite codes?|open source/i);

    const plans = [...pricing.matchAll(/<article class="plan-card">([\s\S]*?)<\/article>/g)].map(match => match[1]);
    assert.equal(plans.length, 3, 'Free, Pro and Support are the three cards');
    assert.match(plans[0], /<h2>Free plan<\/h2>[\s\S]*?Windows app is coming/i);
    assert.doesNotMatch(plans[0], /Available now/i);
    assert.match(plans[0], /local Processing[\s\S]*?your own provider keys/i);
    assert.match(plans[0], /Account[\s\S]*?\d+ free (?:Processed audio |cloud )minutes a month[\s\S]*?capacity lasts/i);
    assert.doesNotMatch(plans[0], /Managed access: Planned/i);
    assert.match(plans[1], /<h2>Pro plan<\/h2>[\s\S]*?Coming soon/i);
    assert.match(plans[1], /<a [^>]*href="https:\/\/github\.com\/ducdv98\/meeting-minutes\/discussions"[^>]*>I'd pay for Pro<\/a>/i);
    assert.match(plans[2], /<h2>Support the project<\/h2>/);
    assert.match(plans[2], /<a [^>]*href="https:\/\/github\.com\/sponsors\/ducdv98"[^>]*>GitHub Sponsors<\/a>/);
    assert.match(pricing, /no Account needed/i);
    assert.match(pricing, /no app-imposed usage limit/i);
    assert.match(pricing, /admitted in order of verification/i);
    assert.match(pricing, /larger Managed transcription allowance/i);
    assert.match(pricing, /Processed audio minutes/i);
    assert.match(pricing, /each Source counts separately/i);
    assert.match(pricing, /Import counts as one stream/i);
    assert.match(pricing, /Re-transcribing the same audio counts again/i);
    assert.match(pricing, /resets on the first of each month \(UTC\)[\s\S]*?without rollover/i);
    assert.match(pricing, /covers transcription only[\s\S]*?Correction[\s\S]*?Summary[\s\S]*?Ask/i);
    assert.match(pricing, /falls back to your own keys[\s\S]*?local Whisper/i);
    assert.match(pricing, /Signing in alone does not turn on Managed transcription/i);
    assert.match(pricing, /used Managed transcription before Accounts[\s\S]*?admitted when (?:it|you) signs? in/i);

    assert.doesNotMatch(pricing, /[$€£]\s*\d|\bprice\b/i);
    assert.doesNotMatch(pricing, /<form\b/i);
    assert.doesNotMatch(pricing, /\b(?:checkout|buy|subscribe|sign up)\b/i);
    assert.doesNotMatch(pricing, /\b(?:open source|faster|more accurate)\b/i);
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
});

test('Free plan is available when a Windows Release is published', async () => {
  const temp = await mkdtemp(join(tmpdir(), 'meeting-minutes-pricing-release-'));
  const output = join(temp, 'site');
  try {
    const fixture = join(temp, 'releases.json');
    await writeFile(fixture, JSON.stringify([{
      tag_name: 'v0.1.0', published_at: '2026-12-01T12:00:00Z', draft: false,
      prerelease: false, body: 'Initial release', assets: [{ name: 'Meeting-Minutes.exe', size: 14680064,
        browser_download_url: 'https://github.com/ducdv98/meeting-minutes/releases/download/v0.1.0/Meeting-Minutes.exe' }],
    }]));
    await run(process.execPath, ['website/build.mjs', '--draft', '--fixture', fixture, '--output', output], { cwd: repo });
    const pricing = await readFile(join(output, 'pricing', 'index.html'), 'utf8');
    const free = pricing.match(/<article class="plan-card">([\s\S]*?)<\/article>/)?.[1];
    assert.ok(free);
    assert.match(free, /<h2>Free plan<\/h2>[\s\S]*?<p class="plan-status">Available now<\/p>/);
    assert.match(free, /free cloud minutes a month/i);
    assert.doesNotMatch(free, /Windows app is coming/);
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
});
