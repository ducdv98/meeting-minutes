import test from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const run = promisify(execFile);
const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

test('changelog contains only published Release notes', async () => {
  const temp = await mkdtemp(join(tmpdir(), 'meeting-minutes-changelog-'));
  try {
    const output = join(temp, 'site');
    await run(process.execPath, ['website/build.mjs', '--fixture', 'website/test/release-fixture.json', '--output', output], { cwd: repo });
    const changelog = await readFile(join(output, 'changelog/index.html'), 'utf8');
    assert.match(changelog, /<h2>v1\.0\.0<\/h2>/);
    assert.match(changelog, /<h3>First release<\/h3>/);
    assert.doesNotMatch(changelog, /Cloud transcription · In development|Work in progress/);
  } finally { await rm(temp, { recursive: true, force: true }); }
});
