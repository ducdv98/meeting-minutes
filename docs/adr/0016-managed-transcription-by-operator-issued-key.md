# Managed transcription by operator-issued key while Accounts are deferred

For the experimental period, Managed transcription is reached with a **Managed key** that the operator issues by hand to people who ask through a request form on the website. This supersedes ADR 0015 and amends ADRs 0012 and 0014. Accounts (email-code sign-in, automatic admission, the Waitlist, Delete account) are deferred, not abandoned: the Account feature is not working today, and it returns when Managed transcription leaves the experimental stage.

- The Provider stays "Managed transcription". Its one Speech-to-text model is shown as `mm-transcription-internal`.
- The user pastes the key into Settings and the app keeps it like any provider key, one secret in the Credential Manager, and sends it directly with each request. The refresh-token exchange and the email field go away. Saving the key is the user's consent to cloud transcription.
- Each key carries the monthly allowance (Processed audio minutes, reset on the first day of each month in UTC, no rollover), and the operator spending ceiling still applies.
- The request form is a pre-filled `mailto:` to the operator. The site has no backend, and the form adds no new service.
- An invalid or revoked key is a visible state in Settings and is not retried until the user edits it. Allowance exhaustion and service pause keep the existing fall-through in the Transcription order.
- The Account UI is hidden in the app and on the site while its code stays dormant.

## Considered Options

- **Finish Accounts for the experimental version:** broader reach, but the feature does not work today and automatic admission needs an email sender and abuse limits.
- **Keep the Invite-code token exchange:** fewer gateway changes, but it keeps a second credential model unlike the other Providers.

## Consequences

- The gateway authenticates and meters by key instead of by Account.
- Pricing and privacy copy are rewritten around "request a key", and the privacy page covers the request email.
- Bringing Accounts back later needs a new ADR on how Accounts relate to Managed keys.
