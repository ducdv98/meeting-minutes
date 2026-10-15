# 12: Delete account

**What to build:** Settings offers Delete account behind a confirmation. It calls the gateway, which removes the email address, usage and every installation's tokens, then signs out locally. Local Meetings are untouched. See `.scratch/v1-launch/spec.md`.

**Blocked by:** 10, gateway 07 (`meeting-minutes-transcription-gateway/.scratch/accounts/issues/`)

Status: ready-for-agent

- [ ] A confirmation explains what is deleted and what stays
- [ ] After deletion the app is signed out and Meetings remain
