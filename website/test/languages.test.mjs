import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

test('language FAQ names every supported Meeting language', async () => {
  const home = await readFile(new URL('../src/home.html', import.meta.url), 'utf8');
  const faq = home.match(/<div class="faq-item"><h3><button[^>]+aria-controls="faq-lang"[\s\S]*?<\/div><\/div><\/div>/)?.[0] ?? '';
  for (const language of ['English', 'Vietnamese', 'Spanish', 'French', 'German', 'Portuguese', 'Italian', 'Japanese', 'Korean', 'Chinese', 'Hindi']) {
    assert.match(faq, new RegExp(language));
  }
  assert.match(faq, /detect/i);
});
