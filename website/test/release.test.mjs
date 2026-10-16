import test from 'node:test';
import assert from 'node:assert/strict';
import { escapeHtml, formatSize, parseInstallerChecksum, publishedReleases, releasePageState, renderNotes } from '../lib/release.mjs';

const release = (overrides = {}) => ({
  tag_name: 'v0.1.0', published_at: '2026-12-01T12:00:00Z', draft: false,
  prerelease: false, body: 'First beta', assets: [{ name: 'Meeting-Minutes.exe', size: 14680064,
    browser_download_url: 'https://github.com/ducdv98/meeting-minutes/releases/download/v0.1.0/Meeting-Minutes.exe' }],
  ...overrides,
});

test('no release keeps download unavailable', () => {
  assert.deepEqual(releasePageState([]), { published: [], latest: null });
});

test('published release exposes its actual installer and size', () => {
  assert.deepEqual(releasePageState([release()]).latest, {
    version: 'v0.1.0', date: '2026-12-01',
    url: 'https://github.com/ducdv98/meeting-minutes/releases/download/v0.1.0/Meeting-Minutes.exe',
    size: '14 MB',
    installerName: 'Meeting-Minutes.exe',
    checksumAsset: null,
  });
});

test('checksum must name the selected installer and contain one SHA-256', () => {
  const hash = 'a'.repeat(64);
  assert.equal(parseInstallerChecksum(`${hash}  Meeting-Minutes.exe\n`, 'Meeting-Minutes.exe'), hash);
  assert.throws(() => parseInstallerChecksum(`${hash}  other.exe`, 'Meeting-Minutes.exe'), /Invalid SHA-256/);
  assert.throws(() => parseInstallerChecksum('not a hash', 'Meeting-Minutes.exe'), /Invalid SHA-256/);
  assert.throws(() => parseInstallerChecksum(`${hash}  Meeting-Minutes.exe\n${hash}  other.exe`, 'Meeting-Minutes.exe'), /Invalid SHA-256/);
});

test('drafts and prereleases are ignored and published releases sort newest first', () => {
  const older = release({ tag_name: 'v0.0.9', published_at: '2026-11-01T00:00:00Z' });
  assert.deepEqual(publishedReleases([release({ draft: true }), older, release({ prerelease: true })]), [older]);
});

test('missing and malformed publication dates are ignored', () => {
  const valid = release();
  assert.deepEqual(publishedReleases([
    release({ published_at: null }),
    release({ published_at: 'not a date' }),
    release({ published_at: '2026-13-01T00:00:00Z' }),
    release({ published_at: '2026-02-30T00:00:00Z' }),
    valid,
  ]), [valid]);
});

test('missing exe asset leaves download unavailable', () => {
  assert.equal(releasePageState([release({ assets: [] })]).latest, null);
  assert.equal(releasePageState([release({ assets: [{ name: 'x.exe', browser_download_url: 'https://evil.example/x.exe' }] })]).latest, null);
});

test('deceptive installer URL is rejected', () => {
  const deceptive = 'https://github.com/ducdv98/meeting-minutes/other/releases/download/v0.1.0/Meeting-Minutes.exe';
  assert.equal(releasePageState([release({ assets: [{ name: 'Meeting-Minutes.exe', browser_download_url: deceptive }] })]).latest, null);
});

test('newest published release without an installer does not expose an older download', () => {
  const older = release({ tag_name: 'v0.1.0', published_at: '2026-12-01T12:00:00Z' });
  const newer = release({ tag_name: 'v0.2.0', published_at: '2027-01-01T12:00:00Z', assets: [] });
  const state = releasePageState([older, newer]);
  assert.deepEqual(state.published, [newer, older]);
  assert.equal(state.latest, null);
});

test('size formatting handles small and invalid sizes', () => {
  assert.equal(formatSize(1536), '2 KB');
  assert.equal(formatSize(1572864), '1.5 MB');
  assert.equal(formatSize(undefined), 'Size unknown');
});

test('notes escape HTML and render only safe paragraphs, lists and links', () => {
  const html = renderNotes('Hello <script>alert(1)</script>\n\n- Fixed [notes](https://github.com/ducdv98/meeting-minutes/releases)\n- [bad](javascript:alert(1))');
  assert.match(html, /&lt;script&gt;/);
  assert.doesNotMatch(html, /<script>|javascript:/);
  assert.match(html, /<ul>\s*<li>Fixed <a href=/);
  assert.equal(escapeHtml('"<&'), '&quot;&lt;&amp;');
});

test('release notes link to a VirusTotal scan only for a SHA-256 file URL', () => {
  const hash = 'a'.repeat(64);
  const html = renderNotes(`[VirusTotal scan](https://www.virustotal.com/gui/file/${hash})\n\n[bad](https://www.virustotal.com/gui/file/not-a-hash)`);
  assert.match(html, new RegExp(`<a href="https://www\\.virustotal\\.com/gui/file/${hash}">VirusTotal scan</a>`));
  assert.doesNotMatch(html, /href="https:\/\/www\.virustotal\.com\/gui\/file\/not-a-hash"/);
});

test('generated verification notes render without workflow markers', () => {
  const html = renderNotes('<!-- installer-verification -->\n## Verify the installer\nSHA-256: abc\n<!-- /installer-verification -->');
  assert.match(html, /<h3>Verify the installer<\/h3>/);
  assert.match(html, /SHA-256: abc/);
  assert.doesNotMatch(html, /installer-verification/);
});
