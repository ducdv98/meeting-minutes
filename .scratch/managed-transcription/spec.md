# Managed transcription spec

Status: ready-for-agent

Wire the app to the experimental Meeting Minutes transcription service (the "gateway"), so an invited user gets cloud transcription without their own Groq key. Vocabulary: `CONTEXT.md` (**Managed transcription**, **Invite code**, **Processing**, **Chunk**, **Vocabulary**, **Meeting language**). Decisions: ADR 0012 (this feature), ADR 0011 (amended), ADR 0010 (background transcription).

The service contract is the handoff doc "Transcription Gateway — Desktop Client Integration Handoff" (https://claude.ai/code/artifact/cade6330-73d3-4a69-998c-e4290bc8fdc9). This spec covers only the app side. The service is a black box.

## Goal

A user pastes an Invite code in Settings. From then on, every Meeting is transcribed by Managed transcription first, during the Recording and after Stop, on any PC that's online. When the service is busy, failing or revoked, the Meeting still finishes on the user's own key, Gemini or local Whisper, and the Meeting view says why.

## Requirements

### Settings and credentials
- A "Managed transcription (experimental)" section in Settings, next to the Groq and Gemini keys, with:
  - a field to paste an Invite code and a Connect button;
  - a status line (Connected / Disconnected) and a Disconnect button;
  - the service's privacy statement under the field.
- Connect calls `POST /v1/auth/activate`. On success, `client_id` and `refresh_token` go in the Credential Manager through the existing secret store. They never go in the settings file. A `401` shows "This invite code is used, expired or unknown. Ask for a new one."
- Connecting is consent to cloud transcription (ADR 0011's rule). There is no separate notice.
- Disconnect clears the stored credentials.
- No onboarding screen and no public copy change.

### Service address
- A core constant, `https://transcribe.meeting-minutes.tech`, overridden by the `MEETING_MINUTES_GATEWAY_URL` environment variable. There is no Settings field.

### Token manager
- One token manager per app process, created lazily from the stored refresh token on first use. The background transcriber and Processing share it.
- The access token is held in memory only, and refreshed through `POST /v1/auth/refresh` 60 s before `expires_in` runs out. The refresh token is not rotated.
- Any `401` on an upload triggers one refresh, then one retry of that upload.
- If refresh itself returns `401`: clear the stored credentials, mark Managed transcription Disconnected, have Settings show "Managed transcription was disconnected", and fall through to the next engine.

### Managed engine
- A `CloudEngine` named "Managed transcription" (new `EngineName::Managed`).
- It receives the chain's span (up to 10 min) and cuts it at pauses into pieces of 170 s or less, using `cloud_span_bounds`.
- Each piece is uploaded to `POST /v1/transcriptions` as 16 kHz mono FLAC (`encode_flac`), with:
  - `request_id`: a new UUID per piece, held in memory and reused only for retries of that piece;
  - `source`: `microphone`, `system` or `unknown`, from the span's Source;
  - `offset_ms`: the piece's start inside the span;
  - `duration_ms`: computed from the piece's sample count;
  - `language`: the Meeting language, always explicit (`en` / `vi`), never `auto`;
  - `model`: `whisper-large-v3`.
- Returned `start_ms` / `end_ms` already include `offset_ms`, so segments are relative to the span, as the `CloudEngine` contract requires. `"segments": []` is a success.
- The Vocabulary is not sent, because the service has no prompt field. Correction still runs.
- The client timeout is at least 120 s. Cancel stops in-flight waits promptly.
- Retries inside the engine:
  - Retryable: `429 RATE_LIMITED`, `409 REQUEST_IN_PROGRESS`, `502`, `503`, `504`, network errors and non-JSON replies. Retry the same piece with the same `request_id` and bytes, up to 3 times. Wait `retry_after_ms` when given, otherwise back off from 2 s, doubling with jitter, capped at 60 s.
  - Not retryable: `400 INVALID_REQUEST`, `400 INVALID_AUDIO`, `409 IDEMPOTENCY_CONFLICT`, `413`, `500`. Log and fail the span at once.
  - Branch on `error.code`, never on the message.
- Errors returned to the chain (the chain's existing policy then applies):
  - retries exhausted on `RATE_LIMITED`, `CAPACITY_EXHAUSTED`, `PROVIDER_UNAVAILABLE` or `TRANSCRIPTION_TIMEOUT` → `QuotaExhausted`, so Managed is skipped for the rest of the run;
  - network unreachable → `Offline`, so during a Recording cloud work pauses as it does for Groq;
  - disconnected (refresh `401`) → `Unauthorized`;
  - not retryable → `Other`.
  The engine never returns `RateLimited` to the chain, because it has already waited.
- Capacity checks (`/v1/capacity`, `suggested_chunk_seconds`) are not used in this version.

### Engine chain
- Order: Managed (when connected) → Groq (own key) → Gemini → local Whisper.
- Auto Meeting language: detect it before the first span with the next engine that can (Groq if a key is set, otherwise local Whisper), then pass it to Managed.
- `background::start_decision`'s `has_groq_key` becomes `has_cloud_engine`, true when connected to Managed or a Groq key is set. Cloud spans then upload during a Recording on any PC.
- The existing fallback notice covers Managed. Wording follows the reason:
  - "…Managed transcription is busy" (Quota)
  - "…Managed transcription was disconnected" (Unauthorized)
  - "…Managed transcription is unavailable" (Offline or Other)

### Diagnostics
- Copy diagnostics shows Managed in engines used and audio per engine. Engine failures log the service's `error.code` and `request_id`. Access tokens, refresh tokens and invite codes never appear in logs or diagnostics.

## Out of scope

- Anything inside the service: provider keys, quotas, pricing.
- Capacity checks and dynamic piece length.
- Persisting `request_id` across app restarts.
- An "accuracy" model setting.
- Public site or README changes.

## Tickets

1. `01-gateway-auth-and-token-manager.md`
2. `02-managed-transcription-in-settings.md`
3. `03-managed-transcription-engine.md`
4. `04-managed-in-the-engine-chain.md`
5. `05-laptop-check-with-a-real-invite.md` (human: needs a real Invite code)
