# Account-based free allowance for Managed transcription

Retire per-install Invite codes when Account access launches. Registration is required for both free and eventual paid Managed transcription, and the free allowance follows the Account across installations. The app has a Free plan with an optional Account, not a separate Registered plan: local Processing and the user's own provider keys remain free without app-imposed usage limits. Only Managed transcription consumes the allowance; Correction, Summary and Ask continue to use local models or the user's own keys. This extends ADR 0012.

The free allowance resets on the first day of each month in UTC without rollover. It counts successfully processed audio per Source, including each re-transcription; an Import counts as one stream. Managed transcription stays first in the automatic transcription order after the user has enabled it, then the user's own keys, then local Whisper. Signing in alone does not consent to uploading audio. If the allowance is exhausted during a Meeting, Processing continues through that order and tells the user where the remaining audio was processed.

Pro is planned as a paid plan with a larger Managed transcription allowance, shown publicly as coming soon without a price, minute count or checkout. The initial operator-wide spending ceiling is $10 per month across model inference, hosting, sign-in and other service costs. The per-Account allowance will be set and published after measuring actual Managed transcription cost, before free Managed access opens. Do not advertise speed or accuracy gains over local transcription until they are measured on English and Vietnamese Meetings.

Anyone may create a free Account once metering and the spending ceiling work, but free Managed access is admitted in cohorts until measured capacity supports wider use. Admit only as many Accounts as the remaining monthly budget can cover at their full published allowance; do not rely on low average usage to make the promise affordable. Sign-in uses a verified email address without an app password; the service retains the email address and usage counters, but does not retain Meeting content long term: audio is deleted after processing and each part's transcribed text is kept for up to five minutes for retries. Free signup does not require a phone number or payment method; use request limits and abuse suspension alongside the total spending ceiling. When the operator-wide ceiling is reached, Managed transcription pauses for everyone until the ceiling resets, without consuming unused Account allowance. The app tells the user that the service is unavailable and continues through the user's own keys or local Whisper.

## Considered Options

- **Keep per-install Invite codes for public access:** simpler than Accounts, but cannot provide one person's allowance across installations.
- **Fund all cloud features from the allowance:** broader convenience, but obscures which usage is being subsidized and expands the service beyond its existing transcription role.

## Consequences

- The service needs Account identity, metering, allowance enforcement and an operator-wide spending stop before open registration.
- The app needs to show remaining allowance, the automatic transcription order and any fallback when Managed transcription is unavailable.
- Public pricing and privacy copy must describe Account data, cohort admission, cloud consent, allowance units and service availability before launch.
- Existing Invite code users must register to keep Managed access; admit them to the first Account cohort without changing their local Meetings.
- If unavoidable fixed costs exceed $10 per month, revise the initial spending ceiling before Managed access launches.
