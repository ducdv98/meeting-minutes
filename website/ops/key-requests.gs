// Google Apps Script bound to the "Key requests" Google Sheet.
// Deploy: Extensions > Apps Script > paste this > Deploy > New deployment > Web app
//   Execute as: Me; Who has access: Anyone. Put the /exec URL in src/request-a-key.html (data-endpoint).
const NOTIFY_TO = 'admin@meeting-minutes.tech';
const HEADERS = ['Received (UTC)', 'Email', 'How they will use it', 'Status', 'Key issued'];

function doPost(e) {
  try {
    const body = JSON.parse(e.postData.contents);
    if (body.website) return reply({ ok: true }); // honeypot: bots fill it, humans never see it
    const email = String(body.email || '').trim().slice(0, 254);
    const use = String(body.use || '').trim().slice(0, 4000);
    if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email) || !use) return reply({ ok: false });

    const sheet = SpreadsheetApp.getActiveSpreadsheet().getSheets()[0];
    if (sheet.getLastRow() === 0) sheet.appendRow(HEADERS);
    // A leading apostrophe stops Sheets from evaluating =formulas typed into the form.
    sheet.appendRow([new Date().toISOString(), "'" + email, "'" + use, 'new', '']);
    MailApp.sendEmail(NOTIFY_TO, 'Managed key request', `${email}\n\n${use}`, { replyTo: email });
    return reply({ ok: true });
  } catch (err) {
    return reply({ ok: false });
  }
}

function reply(obj) {
  return ContentService.createTextOutput(JSON.stringify(obj)).setMimeType(ContentService.MimeType.JSON);
}
