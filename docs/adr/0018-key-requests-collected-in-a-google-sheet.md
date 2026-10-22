# Key requests are collected in a Google Sheet through Apps Script

The request form on the website sends each request to a private Google Sheet, so the operator no longer depends on a visitor's email app to receive it. This amends ADR 0016, which kept the form as a pre-filled `mailto:` link and said it "adds no new service".

- The form posts its email address and answer to a Google Apps Script web app bound to the Sheet. The script appends a row (time, email, use, status, key issued) and emails the operator a copy. The script lives in `website/ops/key-requests.gs`; the web app URL is the form's `data-endpoint`.
- The site stays static. It has no backend of its own and loads no Google script; the browser sends the form directly to Google.
- Google becomes a processor of the request email and answer. The privacy policy says so and links Google's privacy policy.
- The `mailto:` draft stays as the fallback when no endpoint is set or the send fails, so a request is never lost to an outage.
- The script drops submissions that fill a hidden honeypot field, caps field lengths, and stores text so that typed formulas do not run in Sheets.
- The web app URL is public, so the script also limits abuse: a minimum fill time sent by the page, one row per email per six hours, at most 30 rows and 10 notification emails an hour, and a reserve of the daily mail quota. A rejected request still replies `ok:true`, so a script cannot tell which check it hit. The row is written before the email, so a mail failure never loses a request.
- Keys are still issued by hand. The Sheet's status and key-issued columns track that; nothing is issued automatically.

## Considered Options

- **Keep the `mailto:` form:** no new service, but it depends on the visitor having a configured email app and gives the operator no list to work through.
- **A form service such as Formspree:** less code, but a paid or capped third party for one form.
- **A Cloudflare Worker with its own storage:** keeps data with the site host, but adds a backend, storage and abuse limits to maintain.

## Consequences

- The privacy policy's Key requests section describes the Sheet and Google instead of "no form backend".
- The Apps Script deployment sits outside the repository's deploy workflow; changing the script means redeploying it by hand in Google.
- Deleting a request on someone's behalf means deleting their row from the Sheet as well as any email.
