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

test('privacy page says Snapshot images stay on the PC', async () => {
  const policy = await readFile(policyPath, 'utf8');
  assert.match(policy, /Snapshots never leave your PC/i);
  assert.match(policy, /snapshots.*PNG/i);
});

test('Library Ask privacy copy explains local use and cloud confirmation', async () => {
  for (const path of [readmePath, policyPath]) {
    const copy = await readFile(path, 'utf8');
    const row = copy.split('\n').find(line => line.includes('Library Ask') && (line.includes('|') || line.includes('<tr>')));
    assert.ok(row, `${path} needs a Library Ask row`);
    assert.match(row, /Ollama.{0,40}(?:PC|local)/i);
    assert.match(row, /question.{0,120}(?:first|before)/i);
    assert.match(row, /confirm/i);
    assert.match(row, /untick/i);
    assert.match(row, /Transcript/i);
    assert.match(row, /Snapshots? (?:are )?never sent/i);
    assert.match(row, /past Library Asks?.{0,50}Library/i);
  }
});

test('privacy page explains local Microphone app checks and session diagnostics counts', async () => {
  const policy = await readFile(policyPath, 'utf8');
  assert.match(policy, /checks locally which apps use the Microphone[\s\S]*?nothing about that check is sent/i);
  assert.match(policy, /Record prompts shown[\s\S]*?Source health warnings/i);
  assert.match(policy, /counts[\s\S]*?no app names, device names or audio content/i);
});

test('FAQ explains Record prompts and Source health warnings', async () => {
  const home = await readFile(homePath, 'utf8');
  assert.match(home, /Record prompt[\s\S]*?press Record/i);
  assert.match(home, /Source health warnings[\s\S]*?Microphone[\s\S]*?System audio/i);
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

test('privacy page explains key requests and key-based Managed transcription', async () => {
  const policy = await readFile(policyPath, 'utf8');
  assert.match(policy, /Last updated 27 October 2026/);
  assert.match(policy, /Request a key[\s\S]*?private Google Sheet[\s\S]*?email draft/i);
  assert.match(policy, /email address and your description of how you will use Managed transcription/i);
  assert.match(policy, /keys are issued by hand[\s\S]*?do not promise a response or turnaround time/i);
  assert.match(policy, /https:\/\/transcribe\.meeting-minutes\.tech/);
  assert.match(policy, /experimental free service[\s\S]*?agree to the policy of our transcription gateway[\s\S]*?several providers/i);
  assert.doesNotMatch(policy, /forwards (it|the audio) to Groq/i);
  assert.match(policy, /sends the Managed key directly with each request and audio spans as FLAC/i);
  assert.match(policy, /Saving a Managed key is your consent[\s\S]*?no audio is uploaded/i);
  assert.match(policy, /Managed cloud privacy notice beside the key field[\s\S]*?one-time cloud notice when you save your first Groq, Gemini, OpenAI or Anthropic key/i);
  assert.match(policy, /monthly usage allowance measured in Processed audio minutes/i);
  assert.match(policy, /Windows Credential Manager[\s\S]*?one Managed key/i);
  assert.match(policy, /Audio is deleted after processing/i);
  assert.match(policy, /transcribed text.{0,100}deleted within 10 minutes/i);
  assert.doesNotMatch(policy, /Sign-in|six-digit code|Delete account|waitlisted|refresh token|admission order/i);
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
