const REPO = 'ducdv98/meeting-minutes';

export function escapeHtml(value) {
  return String(value ?? '').replace(/[&<>"']/g, character => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  })[character]);
}

export function formatSize(bytes) {
  if (!Number.isFinite(bytes) || bytes < 0) return 'Size unknown';
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1).replace(/\.0$/, '')} MB`;
}

function safeLink(raw) {
  try {
    const url = new URL(raw);
    if (url.protocol !== 'https:') return null;
    if (url.hostname === 'meeting-minutes.tech') return url.href;
    if (url.hostname === 'github.com' && url.pathname.startsWith(`/${REPO}/`)) return url.href;
    if (url.hostname === 'www.virustotal.com' && !url.username && !url.password &&
        !url.search && !url.hash && /^\/gui\/file\/[0-9a-f]{64}$/i.test(url.pathname)) return url.href;
  } catch { /* Text is shown without a link. */ }
  return null;
}

function inlineMarkdown(text) {
  // Escape first. Only links to this product's own site or repository are emitted.
  return escapeHtml(text).replace(/\[([^\]\n]+)\]\(([^\s)]+)\)/g, (whole, label, href) => {
    const safe = safeLink(href.replace(/&amp;/g, '&'));
    return safe ? `<a href="${escapeHtml(safe)}">${label}</a>` : label;
  });
}

export function renderNotes(markdown) {
  const lines = String(markdown ?? '').replace(/\r\n?/g, '\n').split('\n');
  const output = [];
  let paragraph = [];
  let inList = false;
  const flush = () => {
    if (paragraph.length) output.push(`<p>${inlineMarkdown(paragraph.join(' '))}</p>`);
    paragraph = [];
    if (inList) output.push('</ul>');
    inList = false;
  };
  for (const line of lines) {
    if (/^<!-- \/?installer-verification -->$/.test(line.trim())) { flush(); continue; }
    const heading = line.match(/^## (.+)$/);
    if (heading) { flush(); output.push(`<h3>${inlineMarkdown(heading[1])}</h3>`); continue; }
    const item = line.match(/^\s*[-*]\s+(.+)$/);
    if (item) {
      if (paragraph.length) { output.push(`<p>${inlineMarkdown(paragraph.join(' '))}</p>`); paragraph = []; }
      if (!inList) { output.push('<ul>'); inList = true; }
      output.push(`<li>${inlineMarkdown(item[1])}</li>`);
    } else if (!line.trim()) {
      flush();
    } else {
      if (inList) { output.push('</ul>'); inList = false; }
      paragraph.push(line.trim());
    }
  }
  flush();
  return output.join('\n');
}

function validPublishedDate(value) {
  if (typeof value !== 'string') return false;
  const match = value.match(/^(\d{4})-(\d{2})-(\d{2})T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/);
  if (!match || !Number.isFinite(Date.parse(value))) return false;
  const [, year, month, day] = match.map(Number);
  const calendarDay = new Date(Date.UTC(year, month - 1, day));
  return calendarDay.getUTCFullYear() === year && calendarDay.getUTCMonth() + 1 === month && calendarDay.getUTCDate() === day;
}

export function publishedReleases(releases) {
  return releases.filter(release => !release.draft && !release.prerelease && validPublishedDate(release.published_at))
    .sort((a, b) => Date.parse(b.published_at) - Date.parse(a.published_at));
}

export function releasePageState(releases) {
  const published = publishedReleases(releases);
  const prefix = `https://github.com/${REPO}/releases/download/`;
  const hasInstaller = asset => {
    if (!/\.exe$/i.test(asset.name ?? '') || typeof asset.browser_download_url !== 'string' ||
        !asset.browser_download_url.startsWith(prefix)) return false;
    try {
      const url = new URL(asset.browser_download_url);
      return url.origin === 'https://github.com' && !url.username && !url.password &&
        url.pathname.startsWith(`/${REPO}/releases/download/`) && /\.exe$/i.test(url.pathname);
    } catch { return false; }
  };
  const downloadable = published.find(release => release.assets?.some(hasInstaller));
  const installer = downloadable?.assets.find(hasInstaller);
  return {
    published,
    latest: downloadable && installer ? {
      version: downloadable.tag_name,
      date: downloadable.published_at.slice(0, 10),
      url: installer.browser_download_url,
      size: formatSize(installer.size),
    } : null,
  };
}
