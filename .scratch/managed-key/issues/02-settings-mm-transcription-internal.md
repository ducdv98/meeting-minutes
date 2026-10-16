# Settings: mm-transcription-internal key field

Status: ready-for-agent
Spec: ../spec.md
Blocked by: 01

## What
- Managed transcription appears as a Provider with the model `mm-transcription-internal`, using the same key field, saved state and remove button as other Providers.
- Consent and privacy notice beside the field; remaining allowance when reported; invalid/revoked state; fallback notice in the Meeting view.
- Hide Account sign-in, Delete account and Waitlist UI. Add a "Request a key" link to the website.

## Acceptance criteria
- [ ] UI tests: save, remove, invalid-key state, remaining minutes, no Account UI visible.
- [ ] Retire the Invite-code Connect/Disconnect copy.
