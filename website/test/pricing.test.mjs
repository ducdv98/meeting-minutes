import test from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { runInNewContext } from 'node:vm';

const run = promisify(execFile);
const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

async function* htmlFiles(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) yield* htmlFiles(path);
    else if (entry.name.endsWith('.html')) yield path;
  }
}

test('pricing and home describe experimental hand-issued Managed keys', async () => {
  const temp = await mkdtemp(join(tmpdir(), 'meeting-minutes-pricing-'));
  const output = join(temp, 'site');
  try {
    await run(process.execPath, ['website/build.mjs', '--draft', '--offline', '--output', output], { cwd: repo });
    const pricing = await readFile(join(output, 'pricing', 'index.html'), 'utf8');
    const home = await readFile(join(output, 'index.html'), 'utf8');
    const request = await readFile(join(output, 'request-a-key', 'index.html'), 'utf8');
    const support = await readFile(join(output, 'support', 'index.html'), 'utf8');
    const llms = await readFile(join(output, 'llms.txt'), 'utf8');

    for await (const path of htmlFiles(output)) {
      const page = await readFile(path, 'utf8');
      assert.doesNotMatch(page, /invite codes?|waitlist|sign.?in code|admitted in order|refresh token/i,
        `${relative(output, path)} uses current access language`);
      for (const nav of ['Main navigation', 'Mobile navigation', 'Footer navigation']) {
        const content = page.match(new RegExp(`<nav\\b[^>]*aria-label="${nav}"[^>]*>([\\s\\S]*?)<\\/nav>`))?.[1];
        assert.ok(content, `${relative(output, path)} has ${nav}`);
        assert.match(content, /<a href="\/pricing\/"(?: aria-current="page")?>Pricing<\/a>/);
      }
    }

    for (const page of [home, pricing, support]) {
      assert.match(page, /href="\/request-a-key\/"/);
      assert.match(page, /experimental/i);
    }
    assert.match(home, /Is it free\?[\s\S]*?id="faq-free"[\s\S]*?request a key/i);
    assert.match(home, /id="faq-upload"[\s\S]*?Managed key[\s\S]*?Meeting Minutes transcription gateway/);
    assert.match(llms, /^Request a key: https:\/\/meeting-minutes\.tech\/request-a-key\/$/m);
    assert.match(llms, /hand-issued Managed key/i);
    assert.doesNotMatch(llms, /optional Account|waitlist/i);

    const plans = [...pricing.matchAll(/<article class="plan-card">([\s\S]*?)<\/article>/g)].map(match => match[1]);
    assert.equal(plans.length, 2, 'Free and Pro are the plan cards');
    assert.match(plans[0], /<h2>Free plan<\/h2>[\s\S]*?Download unavailable/i);
    assert.match(plans[0], /\$0 forever/);
    assert.match(plans[0], /local Processing[\s\S]*?your own provider keys/i);
    assert.match(plans[0], /Managed transcription[\s\S]*?Keys are issued by hand/i);
    assert.match(plans[0], /href="\/request-a-key\/"/);
    assert.match(plans[1], /<h2>Pro plan<\/h2>[\s\S]*?Planned/i);
    assert.match(pricing, /Processed audio minutes/i);
    assert.match(pricing, /each Source counts separately/i);
    assert.match(pricing, /resets on the first of each month \(UTC\)[\s\S]*?without rollover/i);
    assert.match(pricing, /falls back to your own keys[\s\S]*?local Whisper/i);
    assert.doesNotMatch(pricing, /Account|waitlist|\d+ free cloud minutes/i);
    assert.match(request, /<form id="key-request" action="mailto:admin@meeting-minutes.tech" method="post" enctype="text\/plain" data-endpoint="[^"]*">/);
    assert.match(request, /name="email" type="email"[^>]*required/);
    assert.match(request, /name="use"[^>]*required/);
    assert.match(request, /href="mailto:admin@meeting-minutes.tech\?subject=Managed%20key%20request"/);
    assert.match(request, /cannot promise a reply or a turnaround time/i);
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
});

function loadRequestForm({ endpoint = '', fetchImpl } = {}) {
  let submit;
  const status = { textContent: '' };
  const button = { disabled: false };
  const form = {
    dataset: { endpoint },
    elements: { email: { value: '  person@example.com ' }, use: { value: '  Team meetings & notes  ' }, website: { value: '' } },
    addEventListener(type, listener) { if (type === 'submit') submit = listener; },
    querySelector() { return button; },
    reset() {},
    reportValidity() { throw new Error('valid form should not need a validity report'); },
  };
  const window = { scrollY: 0, innerHeight: 800, location: { href: '' },
    addEventListener() {}, matchMedia() { return { matches: true }; } };
  const document = { documentElement: { classList: { add() {} } },
    querySelector() { return null; }, querySelectorAll() { return []; },
    getElementById(id) { return id === 'key-request' ? form : id === 'key-request-status' ? status : null; } };
  return { form, window, status, button, getSubmit: () => submit, context: { document, window, fetch: fetchImpl } };
}

test('request form opens a pre-filled email draft when no endpoint is set', async () => {
  const script = await readFile(join(repo, 'website/src/main.js'), 'utf8');
  const { window, context, getSubmit } = loadRequestForm();
  runInNewContext(script, context);
  let prevented = false;
  await getSubmit()({ preventDefault() { prevented = true; } });
  assert.equal(prevented, true);
  const url = new URL(window.location.href);
  assert.equal(url.protocol, 'mailto:');
  assert.equal(url.pathname, 'admin@meeting-minutes.tech');
  assert.equal(url.searchParams.get('subject'), 'Managed key request');
  assert.equal(url.searchParams.get('body'), 'Email: person@example.com\n\nHow I will use Managed transcription:\nTeam meetings & notes');
});

test('request form posts to the endpoint and falls back to email when sending fails', async () => {
  const script = await readFile(join(repo, 'website/src/main.js'), 'utf8');
  const calls = [];
  const ok = loadRequestForm({ endpoint: 'https://script.google.com/macros/s/x/exec', fetchImpl: async (url, init) => { calls.push({ url, init }); } });
  runInNewContext(script, ok.context);
  await ok.getSubmit()({ preventDefault() {} });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, 'https://script.google.com/macros/s/x/exec');
  const sent = JSON.parse(calls[0].init.body);
  assert.equal(typeof sent.elapsed, 'number');
  delete sent.elapsed;
  assert.deepEqual(sent, { email: 'person@example.com', use: 'Team meetings & notes', website: '' });
  assert.equal(ok.window.location.href, '');
  assert.match(ok.status.textContent, /request is in/i);
  assert.equal(ok.button.disabled, false);

  const failing = loadRequestForm({ endpoint: 'https://script.google.com/macros/s/x/exec', fetchImpl: async () => { throw new TypeError('offline'); } });
  runInNewContext(script, failing.context);
  await failing.getSubmit()({ preventDefault() {} });
  assert.match(failing.window.location.href, /^mailto:admin@meeting-minutes\.tech/);
});

test('Free plan is available when a Windows Release is published', async () => {
  const temp = await mkdtemp(join(tmpdir(), 'meeting-minutes-pricing-release-'));
  const output = join(temp, 'site');
  try {
    const fixture = join(repo, 'website/test/release-fixture.json');
    await run(process.execPath, ['website/build.mjs', '--draft', '--fixture', fixture, '--output', output], { cwd: repo });
    const pricing = await readFile(join(output, 'pricing', 'index.html'), 'utf8');
    const free = pricing.match(/<article class="plan-card">([\s\S]*?)<\/article>/)?.[1];
    assert.ok(free);
    assert.match(free, /<h2>Free plan<\/h2>[\s\S]*?<p class="plan-status">Available now<\/p>/);
    assert.match(free, /Keys are issued by hand/i);
    assert.doesNotMatch(free, /Download unavailable/);
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
});
