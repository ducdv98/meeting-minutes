# Managed transcription engine

Status: ready-for-human
Spec: ../spec.md
Blocked by: 01

## What

- A `CloudEngine` for Managed transcription, using the shared token manager (ticket 01). Add `EngineName::Managed`, labelled "Managed transcription" and saved and loaded with the Transcript like the other engine names.
- Cut each span at pauses into pieces of 170 s or less with `cloud_span_bounds`. Encode each piece with `encode_flac` and upload it as multipart to `POST /v1/transcriptions` with:
  - `file`;
  - `request_id` (a new UUID per piece, reused only for that piece's retries);
  - `source` (`microphone` / `system` / `unknown`);
  - `offset_ms` (the piece's start within the span);
  - `duration_ms` (from the sample count);
  - `language` (always explicit, never `auto`);
  - `model=whisper-large-v3`.
- Send no other fields. The Vocabulary is not sent.
- Segments come back with `offset_ms` already applied, so the engine returns them relative to the span. An empty `segments` list is a success.
- Client timeout of at least 120 s. Cancel interrupts waits and retries promptly.
- `401` on an upload: refresh once through the token manager, then retry once.
- Retryable codes (`RATE_LIMITED`, `REQUEST_IN_PROGRESS`, `PROVIDER_UNAVAILABLE`, `CAPACITY_EXHAUSTED`, `TRANSCRIPTION_TIMEOUT`, network errors, non-JSON replies) get up to 3 retries with the same `request_id` and bytes. Wait `retry_after_ms` when given, otherwise back off from 2 s, doubling with jitter, capped at 60 s. Use an injectable sleeper so tests don't wait.
- Map the outcome to `EngineError` as the spec says:
  - retries exhausted on busy or provider codes → `QuotaExhausted`;
  - unreachable → `Offline`;
  - disconnected → `Unauthorized`;
  - not retryable → `Other`;
  - never `RateLimited`.
- Log `error.code` and `request_id` on failure, never tokens.

## Acceptance criteria

- [ ] Core tests with a fake `HttpClient`:
  - a 7-minute span becomes 3 pieces of 170 s or less, with correct `offset_ms` and `duration_ms`, and merged segments in span time;
  - the multipart has exactly the allowed fields;
  - empty segments are fine;
  - `401` → refresh → retry succeeds;
  - refresh `401` → `Unauthorized`;
  - `503` with `retry_after_ms` waits that long and reuses the same `request_id`;
  - 3 failed retries → `QuotaExhausted`;
  - `400 INVALID_AUDIO` → `Other` with no retry;
  - non-JSON `502` is retried;
  - cancel during a wait.
- [ ] An old Transcript without `Managed` still loads, and one with it round-trips.
