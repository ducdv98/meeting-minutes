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
    if (releases) {
      const fixture = join(temp, 'releases.json');
      await writeFile(fixture, JSON.stringify(releases));
      args.push('--fixture', fixture);
    } else args.push('--offline');
    await run(process.execPath, args, { cwd: repo });
    for await (const path of publicFiles(output)) {
      const content = await readFile(path, 'utf8');
      const siteCopy = path === join(output, 'changelog', 'index.html')
        ? content.replace(/(<article class="release">[\s\S]*?<div class="notes">)([\s\S]*?)(<\/div><\/article>)/g,
          (article, before, _notes, after) => before.includes('Cloud transcription · In development') ? article : before + after)
        : content;
      assert.doesNotMatch(siteCopy, /beta/i, `${path} contains Beta in site-authored copy`);
      assert.doesNotMatch(siteCopy, /roadmap|December 2026|\(target\)/i,
        `${path} contains a roadmap or target date`);
    }
    await check(output);
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
}

test('Windows download is coming without a date before a published Release', async () => {
  await buildAndCheck(null, async output => {
    const home = await readFile(join(output, 'index.html'), 'utf8');
    assert.match(home, /Windows app is coming/);
    assert.doesNotMatch(home, /Download for Windows/);
    const llms = await readFile(join(output, 'llms.txt'), 'utf8');
    assert.match(llms, /Windows app is coming/);
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

test('public copy has no Beta with a published Windows Release', async () => {
  await buildAndCheck([{
    tag_name: 'v0.1.0', published_at: '2026-12-01T12:00:00Z', draft: false,
    prerelease: false, body: 'First beta', assets: [{ name: 'Meeting-Minutes.exe', size: 14680064,
      browser_download_url: installer }],
  }], async output => {
    const home = await readFile(join(output, 'index.html'), 'utf8');
    assert.ok(home.includes(`href="${installer}"`), 'download links to the published installer');
    assert.match(home, /Version v0\.1\.0 · 14 MB · Windows 11 · Free · unsigned/);
    assert.match(home, /href="\/support\/#smartscreen">see why/);
    const llms = await readFile(join(output, 'llms.txt'), 'utf8');
    assert.doesNotMatch(llms, /roadmap|December 2026|\(target\)/i);
    const changelog = await readFile(join(output, 'changelog', 'index.html'), 'utf8');
    assert.match(changelog, /<div class="notes"><p>First beta<\/p><\/div>/);
  });
});

test('support explains how to verify or build the unsigned installer', async () => {
  await buildAndCheck(null, async output => {
    const support = await readFile(join(output, 'support', 'index.html'), 'utf8');
    assert.match(support, /free and its source is public[\s\S]*unsigned/i);
    assert.match(support, /gh attestation verify [^<]+ --repo ducdv98\/meeting-minutes/);
    assert.match(support, /Get-FileHash/);
    assert.match(support, /\.sha256/);
    assert.match(support, /build it yourself/i);
    assert.match(support, /More info[\s\S]*Run anyway/);
  });
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
