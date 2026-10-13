import { readFile, writeFile, mkdir, rm, copyFile, access } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { escapeHtml, releasePageState, renderNotes } from './lib/release.mjs';

const root = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(root, '..');
const src = join(root, 'src');
const dist = join(root, 'dist');
const args = process.argv.slice(2);
const draft = args.includes('--draft');
const offline = args.includes('--offline');
const checkReady = args.includes('--check-ready');
const fixtureIndex = args.indexOf('--fixture');
if (fixtureIndex !== -1 && !args[fixtureIndex + 1]) { console.error('--fixture needs a JSON file path'); process.exit(1); }
const fixture = fixtureIndex === -1 ? null : resolve(args[fixtureIndex + 1]);
const screens = ['meeting', 'ask', 'transcript', 'search', 'diagnostics', 'firstrun-model'];
const missing = [];

const exists = async path => access(path).then(() => true, () => false);
if (!(await exists(join(src, 'privacy.html')))) missing.push('privacy page: website/src/privacy.html');
else if ((await readFile(join(src, 'privacy.html'), 'utf8')).includes('PRIVACY_STUB_TICKET_14'))
  missing.push('privacy page: website/src/privacy.html still has the ticket 14 stub');
for (const name of screens) for (const theme of ['light', 'dark']) {
  const path = join(src, 'assets', 'screens', `${name}-${theme}.png`);
  if (!(await exists(path))) missing.push(`screenshot: website/src/assets/screens/${name}-${theme}.png`);
}
if (checkReady) {
  const ready = missing.length === 0;
  console.log(ready ? 'Website is ready to publish.' : `Website cannot publish yet:\n- ${missing.join('\n- ')}`);
  if (process.env.GITHUB_OUTPUT) await writeFile(process.env.GITHUB_OUTPUT, `ready=${ready}\n`, { flag: 'a' });
  process.exit(0);
}
if (missing.length && !draft) {
  console.error(`Non-draft website build blocked:\n- ${missing.join('\n- ')}\nUse --draft only for local previews.`);
  process.exit(1);
}

async function getReleases() {
  if (fixture) {
    const data = JSON.parse(await readFile(fixture, 'utf8'));
    if (!Array.isArray(data)) throw new Error('Release fixture must be a JSON array');
    return data;
  }
  if (offline) return [];
  const releases = [];
  let next = 'https://api.github.com/repos/ducdv98/meeting-minutes/releases?per_page=100';
  const visited = new Set();
  while (next) {
    if (visited.has(next)) throw new Error('GitHub Releases pagination repeated a page');
    visited.add(next);
    const response = await fetch(next, {
      headers: { Accept: 'application/vnd.github+json', 'User-Agent': 'meeting-minutes-website-build',
        ...(process.env.GITHUB_TOKEN ? { Authorization: `Bearer ${process.env.GITHUB_TOKEN}` } : {}) },
    });
    if (!response.ok) {
      const reset = response.headers.get('x-ratelimit-reset');
      const resetDate = reset && Number.isFinite(Number(reset))
        ? `${reset} (${new Date(Number(reset) * 1000).toISOString()})` : reset ?? 'unknown';
      const detail = [
        `GitHub Releases request failed: HTTP ${response.status}`,
        ...(response.status === 403 || response.status === 429 ? [
          `rate limit remaining: ${response.headers.get('x-ratelimit-remaining') ?? 'unknown'}`,
          `rate limit reset: ${resetDate}`,
          `Retry-After: ${response.headers.get('retry-after') ?? 'not provided'}`,
        ] : []),
      ];
      throw new Error(detail.join('; '));
    }
    const batch = await response.json();
    if (!Array.isArray(batch)) throw new Error('GitHub Releases response was not an array');
    releases.push(...batch);
    const links = response.headers.get('link') ?? '';
    const nextMatch = links.split(',').map(part => part.trim().match(/^<([^>]+)>;[^,]*\brel="?next"?(?:;|$)/)).find(Boolean);
    next = nextMatch?.[1] ?? null;
    if (next && new URL(next).origin !== 'https://api.github.com') throw new Error('GitHub Releases pagination pointed outside the API');
  }
  return releases;
}

let releaseData;
try { releaseData = await getReleases(); }
catch (error) { console.error(error.message); process.exit(1); }
const { latest, published } = releasePageState(releaseData);
const releaseButton = latest ? `<a class="button primary" href="${escapeHtml(latest.url)}">Download for Windows</a>` : '';
const target = '<span class="status-line"><strong>Free Beta for Windows:</strong> December 2026 (target)</span>';
const heroAction = latest
  ? `${releaseButton}<small>Version ${escapeHtml(latest.version)} · ${escapeHtml(latest.size)} · Windows 11 · Free Beta · unsigned, <a href="/support/#smartscreen">see why</a></small>`
  : `${target}<small><a href="/#roadmap">See the roadmap</a></small>`;
const closingAction = latest ? releaseButton : `${target}<a class="textlink" href="/#roadmap">See the roadmap →</a>`;
const headerAction = latest ? `<a class="button primary header-download" href="${escapeHtml(latest.url)}">Download for Windows <span>${escapeHtml(latest.version)}</span></a>` : '';

function screen(name, alt) {
  const light = `assets/screens/${name}-light.png`;
  const dark = `assets/screens/${name}-dark.png`;
  if (missing.some(item => item.includes(`${name}-`))) {
    return `<div class="screenshot placeholder" role="img" aria-label="${escapeHtml(alt)}. Screenshot pending"><span>Screenshot</span></div>`;
  }
  return `<picture class="screenshot"><source media="(prefers-color-scheme: dark)" srcset="/${dark}"><img src="/${light}" alt="${escapeHtml(alt)}" loading="lazy"></picture>`;
}

const substitutions = {
  '{{HERO_ACTION}}': heroAction, '{{CLOSING_ACTION}}': closingAction,
  '{{SCREEN_MEETING}}': screen('meeting', 'Meeting view with a Transcript and a cited Summary'),
  '{{SCREEN_ASK}}': screen('ask', 'Ask view with an Answer linked to Transcript Citations'),
  '{{SCREEN_TRANSCRIPT}}': screen('transcript', 'Transcript with Speaker labels and playback controls'),
  '{{SCREEN_SEARCH}}': screen('search', 'Library search results with matching Transcript Segments'),
  '{{SCREEN_DIAGNOSTICS}}': screen('diagnostics', 'Help menu with Copy diagnostics selected'),
  '{{SCREEN_FIRSTRUN_MODEL}}': screen('firstrun-model', 'First-run guide recommending a transcription model'),
  '{{WINDOWS_DATE}}': published.length ? new Intl.DateTimeFormat('en', { month: 'long', year: 'numeric', timeZone: 'UTC' }).format(new Date(published.at(-1).published_at)) : 'December 2026 (target)',
  '{{WINDOWS_STATUS}}': published.length ? '<span class="road-status available"><span aria-hidden="true">✓</span> Available</span>' : '<span class="road-status progress"><span aria-hidden="true">◌</span> In progress</span>',
};
function replace(text) {
  for (const [key, value] of Object.entries(substitutions)) text = text.replaceAll(key, value);
  return text;
}

function brand(label = 'Meeting Minutes') {
  return `<picture class="wordmark"><source media="(prefers-color-scheme: dark)" srcset="/brand/wordmark-white.png"><img src="/brand/wordmark-dark.png" width="168" height="24" alt="${label}"></picture>`;
}

function shell(title, description, body, current = '') {
  const links = [['Privacy', '/privacy/'], ['Support', '/support/'], ['Changelog', '/changelog/']];
  const nav = links.map(([label, url]) => `<a href="${url}"${current === url ? ' aria-current="page"' : ''}>${label}</a>`).join('');
  return `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escapeHtml(title)} · Meeting Minutes</title><meta name="description" content="${escapeHtml(description)}">
<meta property="og:type" content="website"><meta property="og:title" content="${escapeHtml(title)} · Meeting Minutes"><meta property="og:description" content="${escapeHtml(description)}"><meta property="og:image" content="https://meeting-minutes.tech/brand/og.png">
<link rel="icon" type="image/png" href="/brand/mark-dark.png"><link rel="stylesheet" href="/style.css"><script src="/main.js" defer></script></head>
<body><a class="skip-link" href="#main">Skip to content</a>
<header class="site-head"><div class="wrap header-inner"><a href="/" class="brand-link" aria-label="Meeting Minutes home">${brand('')}</a><span class="beta">Beta</span>
<nav class="site-nav" aria-label="Main navigation">${nav}${headerAction}</nav>
<details class="mobile-menu"><summary aria-label="Open menu">Menu</summary><nav aria-label="Mobile navigation">${nav}${headerAction}</nav></details></div></header>
<main id="main">${body}</main>
<footer class="site-foot"><div class="wrap footer-grid"><div>${brand()}<p>A meeting recorder for Windows that keeps your meetings on your PC. Made by Duc Dao.</p></div>
<nav aria-label="Footer navigation">${nav}<a href="https://github.com/ducdv98/meeting-minutes">GitHub</a></nav>
<div class="legal"><span>© 2026 Duc Dao</span><a href="mailto:support@meeting-minutes.tech">support@meeting-minutes.tech</a></div></div></footer></body></html>`;
}

async function write(path, value) { await mkdir(dirname(path), { recursive: true }); await writeFile(path, value); }
await rm(dist, { recursive: true, force: true });
await mkdir(dist, { recursive: true });
const home = replace(await readFile(join(src, 'home.html'), 'utf8'));
const support = replace(await readFile(join(src, 'support.html'), 'utf8'));
const privacy = await readFile(join(src, 'privacy.html'), 'utf8').catch(() => '<article class="doc"><h1>Privacy policy</h1><p>Draft pending ticket 14.</p></article>');
const entries = published.map(release => `<article class="release"><h2>${escapeHtml(release.tag_name)}</h2><time datetime="${escapeHtml(release.published_at.slice(0, 10))}">${escapeHtml(new Intl.DateTimeFormat('en', { dateStyle: 'long', timeZone: 'UTC' }).format(new Date(release.published_at)))}</time><div class="notes">${renderNotes(release.body) || '<p>No release notes provided.</p>'}</div></article>`).join('');
const changelog = `<article class="doc"><h1>Changelog</h1><p class="updated">Published releases, newest first.</p>${entries || '<p>No public releases yet. The Windows Free Beta is targeted for December 2026.</p>'}</article>`;
await write(join(dist, 'index.html'), shell('Home', 'Record any call on Windows. Keep a Transcript on your PC and get Answers with Citations.', home));
await write(join(dist, 'support', 'index.html'), shell('Support', 'Get help with Meeting Minutes, diagnostics, installation and uninstalling.', support, '/support/'));
await write(join(dist, 'privacy', 'index.html'), shell('Privacy', 'How Meeting Minutes handles your meetings and data.', privacy, '/privacy/'));
await write(join(dist, 'changelog', 'index.html'), shell('Changelog', 'Published Meeting Minutes releases.', changelog, '/changelog/'));
await write(join(dist, '404.html'), shell('Page not found', 'This page could not be found.', '<article class="doc"><h1>Page not found</h1><p>The page you requested does not exist. <a href="/">Go to the home page</a>.</p></article>'));
await write(join(dist, 'llms.txt'), `# Meeting Minutes\n\nMeeting Minutes is a local-first Windows 11 app by Duc Dao. It records Microphone and System audio from meetings, transcribes English or Vietnamese on the PC, and provides cited Summaries and Answers. No account or telemetry.\n\nWindows Free Beta: ${latest ? `${latest.version}, published ${latest.date}; download: ${latest.url}` : 'December 2026 (target); no public download yet'}.\nMac App Store: December 2026 (target).\n\nHome: https://meeting-minutes.tech/\nPrivacy: https://meeting-minutes.tech/privacy/\nSupport: https://meeting-minutes.tech/support/\nChangelog: https://meeting-minutes.tech/changelog/\nRepository: https://github.com/ducdv98/meeting-minutes\nContact: support@meeting-minutes.tech\n`);
await write(join(dist, 'CNAME'), 'meeting-minutes.tech\n');

const ranges = { latin: 'U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD',
  'latin-ext': 'U+0100-02BA,U+02BD-02C5,U+02C7-02CC,U+02CE-02D7,U+02DD-02FF,U+0304,U+0308,U+0329,U+1D00-1DBF,U+1E00-1E9F,U+1EF2-1EFF,U+2020,U+20A0-20AB,U+20AD-20CF,U+2113,U+2C60-2C7F,U+A720-A7FF',
  vietnamese: 'U+0102-0103,U+0110-0111,U+0128-0129,U+0168-0169,U+01A0-01A1,U+01AF-01B0,U+0300-0301,U+0303-0304,U+0308-0309,U+0323,U+0329,U+1EA0-1EF9,U+20AB' };
let fontCss = '';
for (const [packageName, family, weights] of [['inter', 'Inter', [400, 500, 600]], ['be-vietnam-pro', 'Be Vietnam Pro', [600, 700]]]) {
  for (const weight of weights) for (const subset of ['latin', 'latin-ext', 'vietnamese']) {
    const file = `${packageName}-${subset}-${weight}-normal.woff2`;
    const from = join(repoRoot, 'node_modules', '@fontsource', packageName, 'files', file);
    if (!(await exists(from))) throw new Error(`Required self-hosted font missing: ${from}`);
    await mkdir(join(dist, 'fonts'), { recursive: true });
    await copyFile(from, join(dist, 'fonts', file));
    fontCss += `@font-face{font-family:'${family}';font-style:normal;font-weight:${weight};font-display:swap;src:url('/fonts/${file}') format('woff2');unicode-range:${ranges[subset]}}\n`;
  }
}
await write(join(dist, 'style.css'), fontCss + await readFile(join(src, 'style.css'), 'utf8'));
await copyFile(join(src, 'main.js'), join(dist, 'main.js'));
for (const name of ['wordmark-dark', 'wordmark-white', 'mark-dark', 'mark-white']) {
  await mkdir(join(dist, 'brand'), { recursive: true });
  await copyFile(join(repoRoot, 'src', 'assets', `${name}.png`), join(dist, 'brand', `${name}.png`));
}
await copyFile(join(src, 'assets', 'og.png'), join(dist, 'brand', 'og.png'));
for (const name of screens) for (const theme of ['light', 'dark']) {
  const file = `${name}-${theme}.png`;
  if (await exists(join(src, 'assets', 'screens', file))) {
    await mkdir(join(dist, 'assets', 'screens'), { recursive: true });
    await copyFile(join(src, 'assets', 'screens', file), join(dist, 'assets', 'screens', file));
  }
}
console.log(`Built website/dist (${draft ? 'draft' : 'publishable'}, ${published.length} published releases${missing.length ? `, ${missing.length} placeholders` : ''}).`);
