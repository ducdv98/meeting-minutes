# Managed key spec

Status: ready-for-agent

Replace the Account and Invite-code access to Managed transcription with an operator-issued **Managed key** for the experimental period. Vocabulary: `CONTEXT.md` (**Managed key**, **Managed access**, **Managed transcription**, **Transcription order**). Decision: ADR 0016 (supersedes 0015, amends 0012 and 0014).

## Goal

A user requests a key from the website, pastes it into Settings, and Meetings are transcribed by `mm-transcription-internal` first, as today. Accounts are hidden until later.

## Requirements

### App
- Settings shows Managed transcription as a Provider with one Speech-to-text model, `mm-transcription-internal`, using the same key field, "key saved" state and remove button as the other Providers. The key is stored as one secret in the Credential Manager; the `managed-credential` JSON entry, refresh tokens and email field are no longer used.
- The key is sent directly with each request. There is no activation or refresh exchange.
- The cloud-consent and privacy notice sits next to the field. Saving the key is consent; nothing is uploaded before.
- Settings shows the remaining allowance in minutes when the service reports it.
- An invalid or revoked key shows as its own state in Settings and in the Meeting's fallback notice, and is not retried until the user edits the key. Allowance exhaustion and service pause keep the existing fall-through to the user's own keys and local Whisper, with the notice.
- Account sign-in, Delete account and Waitlist UI are hidden; their code stays dormant. Existing `managed-credential` entries are ignored.
- The Transcription order is unchanged: Managed transcription first once a key is saved.

### Website
- A "Request a key" page with a form (email, how you will use it) that opens a pre-filled `mailto:` to `admin@meeting-minutes.tech`. Linked from home, pricing and the app's Settings.
- Pricing: replace the Account/allowance copy with "Managed transcription is experimental: request a key". Keep "Pro coming soon".
- Privacy: cover the request email and the key-based service data; remove Account sign-in copy.

### Gateway (outside this repo)
- Authenticate and meter by key, report remaining allowance, return a distinct error for an invalid or revoked key.

## Out of scope
- Accounts, automatic admission, Waitlist, a form backend, a key migration from Invite/Account credentials.
