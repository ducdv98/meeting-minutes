# Pricing page and website copy

This is the agreed website design brief. It records decisions for a later implementation; the current website is not changed by this spec. Domain language is in `CONTEXT.md`; the product boundaries are in ADR 0013 and ADR 0014.

## Public presentation

- Remove “Beta” from site-authored pages, build output, `llms.txt`, and website design documentation. Keep the Windows release unavailable until a real published installer exists, and keep December 2026 clearly marked as a target. Keep the factual unsigned-installer guidance.
- Add `/pricing/` and link it from the site's navigation and home page. Follow the existing visual identity and responsive layout.
- Show two plans: **Free** and **Pro**. Registration is an optional Free-plan state, not a third plan.
- Free is $0 forever for local Processing and the user's own provider keys, without an Account or app-imposed usage limit. Explain that a limited monthly Managed transcription allowance is planned for Accounts admitted in cohorts. The actual allowance number is withheld until service cost has been measured and the number can be honored.
- Pro is “Coming soon” with a larger Managed transcription allowance. Show no price, allowance number, payment flow or checkout.
- Do not claim that cloud transcription is faster or more accurate than local Processing until English and Vietnamese Meetings have been benchmarked. Do not call the app open source on the site until a permissive code license has been applied and distribution requirements have been reviewed.

## Current and future availability

- The pricing page may go live before Account registration. Mark Account-based Managed access and Pro as planned, with no signup or waitlist form and no link to a nonexistent flow.
- The current app's experimental Invite code path remains relevant to the privacy page until it is retired. Explain that Managed transcription sends audio to the Meeting Minutes service, which forwards it to Groq; audio is deleted after processing, while each part's transcribed text may be kept for up to five minutes for retries. Account registration and verified-email storage are future policy changes, not current behavior.
- Signing in alone will not enable Managed transcription. Explain that the user will choose to enable it separately.
- Describe the allowance in **Processed audio minutes**: each Source counts separately, so a 60-minute Meeting with Microphone and System audio can use up to 120 minutes. The allowance resets monthly without rollover. When it runs out, the app falls back to the user's own keys and then local Whisper.
- When no public Windows Release exists, show the target date and no download button. When one exists, link to the actual published installer. The Pro card never offers checkout while Pro is unavailable.

## Acceptance criteria for later implementation

- Site-authored public copy contains no “Beta” label, including the header, home, support, privacy, Changelog fallback, and generated `llms.txt`.
- `/pricing/` builds and is reachable from desktop and mobile navigation; the page is readable at the existing supported widths and in light and dark themes.
- Free, optional Account access, and Pro have distinct availability states; no feature, signup or price is presented as live before it exists.
- The privacy page reflects the current Managed transcription flow and its five-minute retry cache, while future Account behavior is clearly labeled as planned.
- Existing release-derived download behavior and SmartScreen guidance remain accurate.

## Values to set later

- Published free Managed transcription allowance, based on measured service cost and the $10 monthly total-service ceiling.
- Exact permissive app-code license, after the dependency and brand-rights review.
- Pro price and allowance, when Pro is ready to launch.
