import test from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const run = promisify(execFile);
const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const installer = 'https://github.com/ducdv98/meeting-minutes/releases/download/v0.1.0/Meeting-Minutes.exe';

async function* publicFiles(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) yield* publicFiles(path);
    else if (entry.name.endsWith('.html') || entry.name === 'llms.txt') yield path;
  }
}

async function buildAndCheck(releases, check) {
  const temp = await mkdtemp(join(tmpdir(), 'meeting-minutes-site-'));
  const output = join(temp, 'site');
  try {
    const args = ['website/build.mjs', '--draft', '--output', output];
    if (releases) {
      const fixture = join(temp, 'releases.json');
      await writeFile(fixture, JSON.stringify(releases));
      args.push('--fixture', fixture);
    } else args.push('--offline');
    await run(process.execPath, args, { cwd: repo });
    for await (const path of publicFiles(output)) {
      assert.doesNotMatch(await readFile(path, 'utf8'), /beta/i, `${path} contains Beta`);
    }
    await check(output);
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
}

test('public copy has no Beta when Windows has no published Release', async () => {
  await buildAndCheck(null, async output => {
    const home = await readFile(join(output, 'index.html'), 'utf8');
    assert.match(home, /Free for Windows:<\/strong> December 2026 \(target\)/);
    assert.doesNotMatch(home, /Download for Windows/);
  });
});

test('public copy has no Beta with a published Windows Release', async () => {
  await buildAndCheck([{
    tag_name: 'v0.1.0', published_at: '2026-12-01T12:00:00Z', draft: false,
    prerelease: false, body: 'Initial release', assets: [{ name: 'Meeting-Minutes.exe', size: 14680064,
      browser_download_url: installer }],
  }], async output => {
    const home = await readFile(join(output, 'index.html'), 'utf8');
    assert.ok(home.includes(`href="${installer}"`), 'download links to the published installer');
    assert.match(home, /Version v0\.1\.0 · 14 MB · Windows 11 · Free · unsigned/);
    assert.match(home, /href="\/support\/#smartscreen">see why/);
  });
});
