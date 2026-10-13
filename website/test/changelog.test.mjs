import test from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { readFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const run = promisify(execFile);
const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

test('changelog labels cloud transcription as in development before a public release', async () => {
  await run(process.execPath, ['website/build.mjs', '--draft', '--offline'], { cwd: repo });
  const changelog = await readFile(join(repo, 'website/dist/changelog/index.html'), 'utf8');
  assert.match(changelog, /Cloud transcription/);
  assert.match(changelog, /In development/);
  assert.match(changelog, /No public releases yet/);
});
