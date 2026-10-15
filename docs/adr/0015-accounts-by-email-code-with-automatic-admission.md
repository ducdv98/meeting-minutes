# Accounts at 1.0, signed in by email code and admitted automatically

Accounts and the free Managed transcription allowance ship in 1.0, not after it, so the launch audience can use Managed transcription. Invite codes are retired at 1.0, and their holders register and are admitted first. This amends ADR 0014 in two ways.

- **Sign-in is a 6-digit code sent by email** and typed into the app. Verifying it returns the same access and refresh tokens that Invite activation returned, so the app's token handling is unchanged. A magic link needs a registered `meeting-minutes://` deep link and fails when the email is opened on another device. OAuth needs a browser round trip and an app registration with each provider.
- **Admission is automatic, not by manual cohort.** Verified Accounts are admitted in order of verification until the configured count N is reached. N comes from the monthly budget and the published allowance, so every admitted Account can use its full allowance. Later Accounts are Waitlisted, can still use local Processing and their own keys, and are emailed when admitted. Manual admission would make the operator a bottleneck during a public launch.

The allowance and the operator ceiling both reset on the first day of each month in UTC, as ADR 0014 says; the gateway's rolling 30-day budget is replaced. The allowance number is provisional (120 Processed audio minutes) during 0.9.0 and is published after that release has measured the cost.

## Consequences

- The gateway needs an email sender. It sits behind a small interface so the provider can change; the first choice is the free tier with the highest daily cap.
- Signup needs abuse limits per email and per IP address, because anyone can register.
- The app offers Delete account, which removes the email address, usage history and every installation's tokens.
