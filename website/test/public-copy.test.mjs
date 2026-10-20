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
const installer = 'https://github.com/ducdv98/meeting-minutes/releases/download/v1.0.0/Meeting-Minutes_1.0.0_x64-setup.exe';
const checksum = '0123456789abcdef'.repeat(4);

function faqLanguageList(home) {
  const answer = home.match(/id="faq-lang"[^>]*><div class="answer-inner">([^<]+)/)?.[1];
  assert.ok(answer, 'home page has a language FAQ answer');
  return answer.match(/^(.+?)\. Choose one for a Meeting or let the app detect it automatically\./)?.[1];
}

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
    if (typeof releases === 'string') args.push('--fixture', releases);
    else if (releases) {
      const fixture = join(temp, 'releases.json');
      await writeFile(fixture, JSON.stringify(releases));
      args.push('--fixture', fixture);
    } else args.push('--offline');
    await run(process.execPath, args, { cwd: repo });
    for await (const path of publicFiles(output)) {
      const content = await readFile(path, 'utf8');
      assert.doesNotMatch(content, /beta|Windows app is coming|Made by|Email Duc Dao|Coming soon/i, `${path} contains outdated copy`);
      assert.doesNotMatch(content, /\bAccounts\b|Meeting Minutes account|create an account/i, `${path} mentions the deferred product identity`);
      assert.doesNotMatch(content, /accuracy|faster than|fastest transcription/i,
        `${path} contains unsupported performance copy`);
      assert.doesNotMatch(content, /roadmap|December 2026|\(target\)/i,
        `${path} contains a roadmap or target date`);
    }
    await check(output);
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
}

test('no Release has no download link or coming claim', async () => {
  await buildAndCheck(null, async output => {
    const home = await readFile(join(output, 'index.html'), 'utf8');
    assert.match(home, /No public download available/);
    assert.doesNotMatch(home, /Download for Windows/);
    const llms = await readFile(join(output, 'llms.txt'), 'utf8');
    assert.match(llms, /No public download available/);
    assert.doesNotMatch(llms, /roadmap|December 2026|\(target\)/i);
  });
});

test('home feature copy and llms.txt agree with the language FAQ', async () => {
  await buildAndCheck(null, async output => {
    const home = await readFile(join(output, 'index.html'), 'utf8');
    const llms = await readFile(join(output, 'llms.txt'), 'utf8');
    const languages = faqLanguageList(home);
    assert.ok(languages);
    assert.ok(home.includes(`Transcription your way</h3><p>${languages}. Auto-detect`));
    assert.ok(llms.includes(`${languages}. Auto-detect`));
  });
});

test('published Windows Release shows installer and SHA-256', async () => {
  await buildAndCheck(join(repo, 'website/test/release-fixture.json'), async output => {
    const home = await readFile(join(output, 'index.html'), 'utf8');
    assert.ok(home.includes(`href="${installer}"`), 'download links to the published installer');
    assert.match(home, /Version v1\.0\.0 · 14 MB · Windows 11, 64-bit · Free/);
    assert.match(home, new RegExp(`SHA-256: <code>${checksum}</code>`));
    assert.match(home, /href="\/support\/#smartscreen">How to verify it/);
    const llms = await readFile(join(output, 'llms.txt'), 'utf8');
    assert.doesNotMatch(llms, /roadmap|December 2026|\(target\)/i);
    const changelog = await readFile(join(output, 'changelog', 'index.html'), 'utf8');
    assert.match(changelog, /<div class="notes"><h3>First release<\/h3>/);
    for (const feature of ['First-run setup can be skipped', 'download or delete local models',
      're-transcribe with one chosen Speech-to-text model', 'Language model for each Ask or Summary',
      'Privacy tab']) assert.match(home, new RegExp(feature, 'i'));
  });
});

test('support explains how to verify or build the unsigned installer', async () => {
  await buildAndCheck(null, async output => {
    const support = await readFile(join(output, 'support', 'index.html'), 'utf8');
    assert.match(support, /free and its source is public[\s\S]*unsigned/i);
    assert.match(support, /Verify your download/);
    assert.match(support, /gh attestation verify &lt;installer&gt; --repo ducdv98\/meeting-minutes/);
    assert.match(support, /Get-FileHash/);
    assert.match(support, /\.sha256/);
    assert.match(support, /build it yourself/i);
    assert.match(support, /More info[\s\S]*Run anyway/);
  });
});

test('published build fails if the matching checksum is missing or unreadable', async () => {
  const temp = await mkdtemp(join(tmpdir(), 'meeting-minutes-checksum-'));
  try {
    const base = JSON.parse(await readFile(join(repo, 'website/test/release-fixture.json'), 'utf8'));
    for (const kind of ['missing', 'malformed']) {
      const releases = structuredClone(base);
      if (kind === 'missing') releases[0].assets.pop();
      else releases[0].assets[1].fixture_path = 'bad.sha256';
      const fixture = join(temp, `${kind}.json`);
      await writeFile(fixture, JSON.stringify(releases));
      if (kind === 'malformed') await writeFile(join(temp, 'bad.sha256'), 'not a checksum\n');
      await assert.rejects(run(process.execPath,
        ['website/build.mjs', '--fixture', fixture, '--output', join(temp, kind)], { cwd: repo }),
      error => {
        assert.match(error.stderr, /SHA-256 (?:asset missing|unreadable)/);
        return true;
      });
    }
  } finally { await rm(temp, { recursive: true, force: true }); }
});

test('build refuses output paths that could delete website sources', async () => {
  const website = join(repo, 'website');
  for (const output of [dirname(repo), repo, website, join(website, 'src'), join(website, 'src', 'nested')]) {
    await assert.rejects(
      run(process.execPath, ['website/build.mjs', '--draft', '--offline', '--check-ready', '--output', output], { cwd: repo }),
      error => {
        assert.equal(error.code, 1, `expected exit 1 for ${output}`);
        assert.match(error.stderr, /Refusing --output path/);
        return true;
      },
    );
  }
});
