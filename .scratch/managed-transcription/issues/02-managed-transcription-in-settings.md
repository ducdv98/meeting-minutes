# Managed transcription in Settings

Status: ready-for-agent
Spec: ../spec.md
Blocked by: 01

## What

- A "Managed transcription (experimental)" section in Settings, next to the Groq and Gemini keys:
  - a field to paste an Invite code and a Connect button;
  - a status line (Connected / Disconnected) and a Disconnect button;
  - the privacy statement from the spec's handoff doc under the field.
- Connect calls `activate` (ticket 01) through a Tauri command and stores the credentials.
  - A `401` shows "This invite code is used, expired or unknown. Ask for a new one."
  - A network failure shows a retryable error.
- Disconnect clears the stored credentials.
- When the token manager reports Disconnected (revoked), the section shows "Managed transcription was disconnected".
- Connecting is consent to cloud transcription. There is no extra notice or dialog.

## Acceptance criteria

- [ ] UI tests: connect success shows Connected; `401` shows the invite error; Disconnect returns to the empty field; a revoked state shows the disconnected message.
- [ ] Nothing about Managed transcription is written to the settings file except, at most, a non-secret "connected" flag.
- [ ] `cargo clippy` for the Windows target passes on `src-tauri/src/main.rs` (see the stub-whisper check).
