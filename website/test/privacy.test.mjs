import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const policyPath = join(repo, 'website/src/privacy.html');
const homePath = join(repo, 'website/src/home.html');
const readmePath = join(repo, 'README.md');
const urlHosts = text => [...text.matchAll(/https?:\/\/([a-z0-9.-]+)(?::\d+)?/gi)]
  .map(match => match[1].toLowerCase());

async function* appFiles(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    // Test code and fixtures may use fake provider URLs; they are not app destinations.
    if (entry.isDirectory()) {
      if (!['test', 'tests', 'fixtures', 'target'].includes(entry.name)) yield* appFiles(path);
    } else if (entry.isFile() && (entry.name.endsWith('.rs') ||
      (dir === join(repo, 'src-tauri') && /^tauri(?:\..+)?\.conf\.json$/.test(entry.name)))) {
      yield path;
    }
  }
}

function runtimeConfig(config) {
  // $schema is an editor reference; build.devUrl is used only by Tauri's dev server.
  const { $schema: _schema, build, ...rest } = config;
  const { devUrl: _devUrl, ...productionBuild } = build ?? {};
  return { ...rest, build: productionBuild };
}

test('privacy page has replaced the ticket stub', async () => {
  const policy = await readFile(policyPath, 'utf8');
  assert.doesNotMatch(policy, /PRIVACY_STUB_TICKET_14/);
});

test('public copy explains when Meeting content goes to cloud providers', async () => {
  for (const path of [policyPath, homePath, readmePath]) {
    const copy = await readFile(path, 'utf8');
    assert.match(copy, /local(?:ly)?(?: by default)?/i, `${path} should describe local Processing`);
    assert.match(copy, /Groq/, `${path} should name Groq transcription`);
    assert.match(copy, /Gemini/, `${path} should name Gemini fallback and Correction`);
    assert.doesNotMatch(copy, /audio never leaves your PC|transcription stays on your PC/i);
  }
  const policy = await readFile(policyPath, 'utf8');
  assert.match(policy, /Gemini.{0,100}free tier.{0,100}training/is);
  assert.match(policy, /Windows Credential Manager/);
});

test('privacy page explains current Account sign-in and Managed transcription', async () => {
  const policy = await readFile(policyPath, 'utf8');
  assert.match(policy, /https:\/\/transcribe\.meeting-minutes\.tech/);
  assert.match(policy, /verified email address[\s\S]*?six-digit code/i);
  assert.match(policy, /Signing in alone does not enable Managed transcription/i);
  assert.match(policy, /Meeting Minutes service[\s\S]*?forwards? (?:the )?audio to Groq/i);
  assert.match(policy, /Audio is deleted after processing/i);
  assert.match(policy, /transcribed text.{0,100}deleted within 10 minutes/i);
  assert.match(policy, /Windows Credential Manager[\s\S]*?client ID, refresh token, Account email address and enabled flag/i);
  assert.match(policy, /Resend[\s\S]*?email address/i);
  assert.match(policy, /waitlisted[\s\S]*?emailed when admitted/i);
  assert.match(policy, /Sign out deletes this installation/i);
  assert.doesNotMatch(policy, /Choosing Disconnect/i);
});

test('privacy page describes stored Account data, admission order and deletion as current', async () => {
  const policy = await readFile(policyPath, 'utf8');
  assert.match(policy, /<h2>Accounts and Managed transcription<\/h2>/);
  assert.match(policy, /verified email address/i);
  assert.match(policy, /monthly usage counters?[^.]*Processed audio minutes/i);
  assert.match(policy, /admitted in order of verification[^.]*capacity lasts/i);
  assert.match(policy, /Delete account in the app[\s\S]*?This removes your email address, usage history and every installation.s tokens/i);
  assert.doesNotMatch(policy, /planned: accounts|account registration is planned|will use a verified email/i);
  assert.doesNotMatch(policy, /invite codes?/i);
});

test('privacy page names every URL host in app source and runtime config', async () => {
  const policy = await readFile(policyPath, 'utf8');
  const networkTable = policy.split('<h2>What the app connects to</h2>')[1]
    ?.split('<h2>What is stored, and where</h2>')[0];
  assert.ok(networkTable, 'Privacy page needs the app connection table');
  const namedDestinations = [...networkTable.matchAll(/<tr><td>([\s\S]*?)<\/td>/g)]
    .map(match => match[1].toLowerCase()).join('\n');
  const missing = [];
  for (const root of ['crates', 'src-tauri']) {
    for await (const path of appFiles(join(repo, root))) {
      let source = await readFile(path, 'utf8');
      if (path.endsWith('.json')) source = JSON.stringify(runtimeConfig(JSON.parse(source)));
      else {
        // Rust doc comments often contain example URLs; they do not make requests.
        source = source.replace(/\/\*\*[\s\S]*?\*\//g, '').replace(/^\s*\/\/[/!].*$/gm, '');
      }
      for (const host of new Set(urlHosts(source))) {
        if (!namedDestinations.includes(host)) missing.push(`${host} (${relative(repo, path)})`);
      }
    }
  }
  assert.deepEqual(missing, [], `App URL hosts missing from privacy policy: ${missing.join(', ')}`);
});
