# 10: Sign in with an email code

**What to build:** Replace the Invite code field in Settings with Account sign-in: enter an email, receive a 6-digit code, enter it. Verifying stores the returned tokens the same way Invite activation did. Signing in does not enable Managed transcription: enabling it is a separate, explicit switch (consent). Sign out clears the tokens. Remove every Invite code UI and string. See `.scratch/v1-launch/spec.md`.

**Blocked by:** gateway 02 (`meeting-minutes-transcription-gateway/.scratch/accounts/issues/`)

Status: ready-for-agent

- [x] Email → code → signed in, with clear errors for a wrong code, an expired code and rate limiting
- [x] Managed transcription stays off until the user turns it on
- [x] Sign out clears the stored credentials
- [x] No Invite code UI remains
