import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

test('core, frontend and website support the same Meeting languages', async () => {
  const [rust, frontend, home] = await Promise.all([
    readFile(join(repo, 'crates/core/src/transcript.rs'), 'utf8'),
    readFile(join(repo, 'src/languages.ts'), 'utf8'),
    readFile(join(repo, 'website/src/home.html'), 'utf8'),
  ]);

  const all = rust.match(/pub const ALL:\s*\[Self;\s*\d+\]\s*=\s*\[([\s\S]*?)\];/)?.[1];
  assert.ok(all, 'Language::ALL exists');
  const variants = [...all.matchAll(/Self::(\w+)/g)].map(match => match[1]);
  const codes = new Map([...rust.matchAll(/Self::(\w+) => "([a-z]+)"/g)].map(match => [match[1], match[2]]));
  const names = new Map([...rust.matchAll(/Self::(\w+) => "([A-Z][a-z]+)"/g)].map(match => [match[1], match[2]]));
  const coreLanguages = variants.map(variant => ({ code: codes.get(variant), name: names.get(variant) }));
  assert.ok(coreLanguages.every(language => language.code && language.name));

  const list = frontend.match(/export const MEETING_LANGUAGES = \[([\s\S]*?)\] as const;/)?.[1];
  assert.ok(list, 'frontend has one Meeting language list');
  const frontendLanguages = [...list.matchAll(/\{ code: "([a-z]+)", name: "([^"]+)"/g)]
    .map(match => ({ code: match[1], name: match[2] }));
  assert.deepEqual(frontendLanguages, coreLanguages);

  const answer = home.match(/id="faq-lang"[^>]*><div class="answer-inner">([^<]+)/)?.[1];
  assert.ok(answer, 'website has a language FAQ answer');
  const namesInFaq = answer.split('. Choose one for a Meeting')[0]
    .replace(/ and /g, ', ').split(', ');
  assert.deepEqual(namesInFaq, coreLanguages.map(language => language.name));
});
