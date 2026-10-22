// Google Apps Script bound to the "Key requests" Google Sheet.
// Deploy: Extensions > Apps Script > paste this > Deploy > New deployment > Web app
//   Execute as: Me; Who has access: Anyone. Put the /exec URL in src/request-a-key.html (data-endpoint).
// The URL is public, so anyone can post to it. The limits below keep that from filling the Sheet or the mail quota.
const NOTIFY_TO = 'admin@meeting-minutes.tech';
const HEADERS = ['Received (UTC)', 'Email', 'How they will use it', 'Status', 'Key issued'];
const MAX_BODY = 10000;          // characters of raw request body
const MIN_FILL_MS = 3000;        // the form is never filled faster than this by a person
const MAX_FILL_MS = 24 * 3600 * 1000;
const MAX_ROWS_PER_HOUR = 30;    // all senders together
const MAX_MAIL_PER_HOUR = 10;    // notification emails, all senders together
const MAIL_QUOTA_RESERVE = 20;   // daily MailApp quota always left for other mail
const DEDUPE_SECONDS = 21600;    // CacheService's maximum lifetime (6 h)

// Every rejection of a request that is not a visitor's typo replies ok:true, so a script cannot tell which check it hit.
function processRequest(raw, env) {
  let body;
  try {
    if (String(raw).length > MAX_BODY) return { ok: false };
    body = JSON.parse(raw);
  } catch (err) {
    return { ok: false };
  }
  if (!body || typeof body !== 'object') return { ok: false };
  if (body.website) return { ok: true }; // honeypot: bots fill it, humans never see it
  const email = String(body.email || '').trim().slice(0, 254);
  const use = String(body.use || '').trim().slice(0, 4000);
  if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email) || !use) return { ok: false };
  const elapsed = body.elapsed;
  if (typeof elapsed !== 'number' || !(elapsed >= MIN_FILL_MS && elapsed <= MAX_FILL_MS)) return { ok: true };

  const hour = Math.floor(env.now / 3600000);
  const rowsKey = 'rows:' + hour;
  const mailKey = 'mail:' + hour;
  const emailKey = 'email:' + email.toLowerCase();
  let sendMail = false;
  env.lock(() => {
    if (env.cache.get(emailKey)) return; // already recorded; a repeat adds nothing
    const rows = Number(env.cache.get(rowsKey) || 0);
    if (rows >= MAX_ROWS_PER_HOUR) return;
    const mails = Number(env.cache.get(mailKey) || 0);
    sendMail = mails < MAX_MAIL_PER_HOUR && env.mailQuota() > MAIL_QUOTA_RESERVE;
    env.cache.put(emailKey, '1', DEDUPE_SECONDS);
    env.cache.put(rowsKey, String(rows + 1), 3600);
    if (sendMail) env.cache.put(mailKey, String(mails + 1), 3600);
    // A leading apostrophe stops Sheets from evaluating =formulas typed into the form.
    env.appendRow([new Date(env.now).toISOString(), "'" + email, "'" + use, 'new', '']);
  });
  if (sendMail) {
    // A mail failure must not lose the request: the row is already in the Sheet.
    try { env.sendMail(email, use); } catch (err) { /* the Sheet is the source of truth */ }
  }
  return { ok: true };
}

function doPost(e) {
  try {
    const sheet = SpreadsheetApp.getActiveSpreadsheet().getSheets()[0];
    const cache = CacheService.getScriptCache();
    return reply(processRequest(e.postData.contents, {
      now: Date.now(),
      cache,
      lock: fn => {
        const lock = LockService.getScriptLock();
        lock.waitLock(10000);
        try { fn(); } finally { lock.releaseLock(); }
      },
      mailQuota: () => MailApp.getRemainingDailyQuota(),
      appendRow: row => {
        if (sheet.getLastRow() === 0) sheet.appendRow(HEADERS);
        sheet.appendRow(row);
      },
      sendMail: (email, use) => MailApp.sendEmail(NOTIFY_TO, 'Managed key request', `${email}\n\n${use}`, { replyTo: email }),
    }));
  } catch (err) {
    return reply({ ok: false });
  }
}

function reply(obj) {
  return ContentService.createTextOutput(JSON.stringify(obj)).setMimeType(ContentService.MimeType.JSON);
}
