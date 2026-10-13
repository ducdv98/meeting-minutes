# Engine chain with local fallback

Status: ready-for-human
Spec: ../spec.md
Blocked by: 02

## What

- Processing, after Stop or on Re-transcribe, transcribes each span through an ordered chain: Groq when a key is set, then Gemini (ticket 05 adds it), then the existing local engine.
- Per span:
  - `RateLimited`: wait for `retry_after`, at most about 60 s, and retry the same engine.
  - `QuotaExhausted`, `Unauthorized` or `Offline`: move to the next engine, and skip the exhausted engine for the rest of the run.
  - `Other`: retry once, then move on.
- Load the local model only when a span actually falls back to it.
- A Meeting may mix engines in one Revision. Record the engines used in the Transcript, extending its `model` field into a list.
- If anything fell back, the Meeting view shows a one-line notice with the reason.
- Diagnostics show the audio each engine did, fallback reasons and per-span round-trip time (spec, Diagnostics).
- Cancel stops in-flight HTTP waits promptly.

## Acceptance criteria

- [ ] Core tests with fake engines:
  - Groq only;
  - Groq quota hit midway, so the rest goes local and one Revision results;
  - no key, so everything goes local and no HTTP call is made;
  - a rate limit is retried;
  - cancel during a retry wait.
- [ ] The fallback notice appears in the Meeting view (UI test).
- [ ] Without any keys, Processing output and diagnostics are unchanged from today, apart from the new fields.
