import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { runInNewContext } from 'node:vm';

const repo = join(dirname(fileURLToPath(import.meta.url)), '../..');
const script = await readFile(join(repo, 'website/ops/key-requests.gs'), 'utf8');
const { processRequest } = (() => {
  const context = {};
  runInNewContext(script + '\nthis.processRequest = processRequest;', context);
  return context;
})();

const T0 = Date.UTC(2026, 9, 22, 12, 0, 0);

function makeEnv({ now = T0, quota = 100, mailFails = false, cache = new Map() } = {}) {
  const rows = [];
  const mails = [];
  return {
    rows, mails, cache,
    env: {
      now,
      cache: { get: k => cache.get(k) ?? null, put: (k, v) => cache.set(k, v) },
      lock: fn => fn(),
      mailQuota: () => quota,
      appendRow: row => rows.push(row),
      sendMail: (email, use) => { if (mailFails) throw new Error('quota'); mails.push({ email, use }); },
    },
  };
}

const good = (over = {}) => JSON.stringify({ email: 'a@example.com', use: 'notes', website: '', elapsed: 20000, ...over });

test('a valid request is stored with a text-forcing apostrophe and notified once', () => {
  const { env, rows, mails } = makeEnv();
  assert.deepEqual(processRequest(good({ email: ' A@Example.com ', use: '=HYPERLINK("x")' }), env), { ok: true });
  assert.equal(rows.length, 1);
  assert.deepEqual(rows[0].slice(1), ["'A@Example.com", `'=HYPERLINK("x")`, 'new', '']);
  assert.equal(rows[0][0], '2026-10-22T12:00:00.000Z');
  assert.deepEqual(mails, [{ email: 'A@Example.com', use: '=HYPERLINK("x")' }]);
});

test('malformed or invalid input reports ok:false and stores nothing', () => {
  const { env, rows } = makeEnv();
  for (const raw of ['not json', 'null', good({ email: 'nope' }), good({ use: '  ' }), good({ email: 'a@b.c\nBcc: x@y.z' }), 'x'.repeat(10001)]) {
    assert.deepEqual(processRequest(raw, env), { ok: false }, raw.slice(0, 40));
  }
  assert.equal(rows.length, 0);
});

test('honeypot, too-fast, missing and stale timers are dropped with ok:true', () => {
  const { env, rows } = makeEnv();
  for (const over of [{ website: 'http://spam' }, { elapsed: 500 }, { elapsed: undefined }, { elapsed: '20000' }, { elapsed: 25 * 3600 * 1000 }]) {
    assert.deepEqual(processRequest(good(over), env), { ok: true }, JSON.stringify(over));
  }
  assert.equal(rows.length, 0);
});

test('the same email is recorded once', () => {
  const { env, rows, mails } = makeEnv();
  processRequest(good(), env);
  processRequest(good({ email: 'A@EXAMPLE.com' }), env);
  assert.equal(rows.length, 1);
  assert.equal(mails.length, 1);
});

test('rows are capped per hour across senders and the cap resets next hour', () => {
  const cache = new Map();
  const first = makeEnv({ cache });
  for (let i = 0; i < 40; i++) processRequest(good({ email: `u${i}@example.com` }), first.env);
  assert.equal(first.rows.length, 30);
  const later = makeEnv({ cache, now: T0 + 3600000 });
  processRequest(good({ email: 'late@example.com' }), later.env);
  assert.equal(later.rows.length, 1);
});

test('notification mail is capped per hour but rows are still stored', () => {
  const { env, rows, mails } = makeEnv();
  for (let i = 0; i < 15; i++) processRequest(good({ email: `u${i}@example.com` }), env);
  assert.equal(rows.length, 15);
  assert.equal(mails.length, 10);
});

test('no notification mail is sent once the daily quota is down to the reserve', () => {
  const { env, rows, mails } = makeEnv({ quota: 20 });
  processRequest(good(), env);
  assert.equal(rows.length, 1);
  assert.equal(mails.length, 0);
});

test('a failing mail send still keeps the row and replies ok:true', () => {
  const { env, rows } = makeEnv({ mailFails: true });
  assert.deepEqual(processRequest(good(), env), { ok: true });
  assert.equal(rows.length, 1);
});
