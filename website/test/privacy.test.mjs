import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const policyPath = join(repo, 'website/src/privacy.html');
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
