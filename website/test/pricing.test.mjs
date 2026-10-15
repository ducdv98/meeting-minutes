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

test('pricing is reachable and describes current and planned access', async () => {
  const temp = await mkdtemp(join(tmpdir(), 'meeting-minutes-pricing-'));
  const output = join(temp, 'site');
  try {
    await run(process.execPath, ['website/build.mjs', '--draft', '--offline', '--output', output], { cwd: repo });
    const pricing = await readFile(join(output, 'pricing', 'index.html'), 'utf8');
    const home = await readFile(join(output, 'index.html'), 'utf8');
    const llms = await readFile(join(output, 'llms.txt'), 'utf8');

    for await (const path of htmlFiles(output)) {
      const page = await readFile(path, 'utf8');
      for (const nav of ['Main navigation', 'Mobile navigation', 'Footer navigation']) {
        const content = page.match(new RegExp(`<nav\\b[^>]*aria-label="${nav}"[^>]*>([\\s\\S]*?)<\\/nav>`))?.[1];
        assert.ok(content, `${relative(output, path)} has ${nav}`);
        assert.match(content, /<a href="\/pricing\/"(?: aria-current="page")?>Pricing<\/a>/,
          `${relative(output, path)} links to pricing in ${nav}`);
      }
    }

    assert.match(home, /Is it free\?[\s\S]*?id="faq-free"[\s\S]*?<a href="\/pricing\/">/);
    assert.match(home, /id="faq-upload"[\s\S]*?Invite code[\s\S]*?Meeting Minutes service, which forwards it to Groq[\s\S]*?<a href="\/privacy\/">/);
    assert.match(llms, /^Pricing: https:\/\/meeting-minutes\.tech\/pricing\/$/m);

    const plans = [...pricing.matchAll(/<article class="plan-card">([\s\S]*?)<\/article>/g)].map(match => match[1]);
    assert.equal(plans.length, 2, 'Free and Pro are the only plan cards');
    assert.match(plans[0], /<h2>Free plan<\/h2>[\s\S]*?Windows app is coming/i);
    assert.doesNotMatch(plans[0], /Available now/i);
    assert.match(plans[0], /Account[\s\S]*?Managed transcription allowance[\s\S]*?Managed access: Planned/i);
    assert.match(plans[1], /<h2>Pro plan<\/h2>[\s\S]*?Coming soon/i);
    for (const plan of plans) assert.doesNotMatch(plan, /\b\d+\s+(?:Processed audio )?minutes?\b/i);
    assert.match(pricing, /\$0 forever/);
    assert.match(pricing, /no Account needed/i);
    assert.match(pricing, /no app-imposed usage limit/i);
    assert.match(pricing, /cohorts/i);
    assert.match(pricing, /published once .*cost .*measured/i);
    assert.match(pricing, /larger Managed transcription allowance/i);
    assert.match(pricing, /Processed audio minutes/i);
    assert.match(pricing, /Planned: how the allowance will work/i);
    assert.match(pricing, /each Source will count separately/i);
    assert.match(pricing, /60-minute Meeting[\s\S]*?Microphone[\s\S]*?System audio[\s\S]*?120 minutes/i);
    assert.match(pricing, /Import will count as one stream/i);
    assert.match(pricing, /Re-transcribing the same audio will count again/i);
    assert.match(pricing, /will reset on the first of each month \(UTC\)[\s\S]*?without rollover/i);
    assert.match(pricing, /will cover transcription only[\s\S]*?Correction[\s\S]*?Summary[\s\S]*?Ask/i);
    assert.match(pricing, /will fall back to your own keys[\s\S]*?local Whisper/i);
    assert.match(pricing, /Signing in alone will not turn on Managed transcription/i);
    assert.match(pricing, /will choose to enable it separately/i);

    for (const amount of pricing.match(/[$€£]\s*\d+(?:[.,]\d+)?/g) ?? []) assert.equal(amount, '$0');
    assert.doesNotMatch(pricing, /<form\b/i);
    assert.doesNotMatch(pricing, /\b(?:checkout|buy|subscribe|sign up|waitlist)\b/i);
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
    assert.match(free, /\$0 forever/);
    assert.doesNotMatch(free, /Windows app is coming/);
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
});
