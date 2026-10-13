# Groq transcription engine

Status: ready-for-human
Spec: ../spec.md
Blocked by: 01

## What

- Add a cloud `SpeechEngine` for Groq that calls the OpenAI-compatible `POST /openai/v1/audio/transcriptions` with:
  - `model=whisper-large-v3`;
  - `response_format=verbose_json`;
  - `timestamp_granularities[]=word` and `timestamp_granularities[]=segment`;
  - `language` from the Meeting language;
  - `prompt` from the Vocabulary, trimmed to Groq's prompt limit.
- Go through the existing `HttpClient` trait (`crates/core/src/ai/http.rs`) so tests can use a fake. Extend it for multipart upload if it can't already do that.
- Encode each span as 16 kHz mono FLAC, or WAV if FLAC is impractical. Assert the payload stays under 25 MB, and split it if not.
- Map the response to `RawSegment` and `RawWord` with millisecond times relative to the span, so `segment_sentences` and stitching work unchanged.
- Classify errors into a typed result that ticket 03 can act on:
  - `RateLimited { retry_after }`, for per-minute 429s;
  - `QuotaExhausted`, for a daily-limit 429;
  - `Unauthorized`;
  - `Offline`;
  - `Other`.
- `backend()` returns `"Groq"`, and `device()` returns the model name.

## Acceptance criteria

- [ ] Fake-HTTP tests: request fields, the verbose_json → RawSegment/RawWord mapping, and each error class.
- [ ] An ignored live test transcribes one FLEURS clip with `GROQ_API_KEY` from the environment.
- [ ] No key or audio content is logged.
