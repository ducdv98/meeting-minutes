# Cloud transcription spec

Status: ready-for-human

Make Transcripts more accurate and faster on any laptop, at no cost to us, by using each user's own free Groq and Gemini API keys. Vocabulary: `CONTEXT.md` (**Processing**, **Correction**, **Segment**, **Draft**, **Revision**, **Chunk**, **Vocabulary**). Decisions: ADR 0011 (this feature), ADR 0002 (amended), ADR 0008 (superseded for cloud), ADR 0010 (background transcription).

## Goal

A user with a free Groq key stops a one-hour call on any laptop that's online and sees the finished, corrected Transcript within about a minute. It's more accurate than local turbo `q5_0`, and both we and the user pay nothing. Without keys the app works exactly as today.

## Budget

We spend $0. The app never ships or proxies a provider key, and there's no backend. Each user's allowance comes from their own free account: Groq is about 8 audio hours a day, Gemini's free tier varies. Using several accounts to get around a provider's limits is out of scope (ADR 0011).

## Requirements

### Keys and setup
- Settings gains a **Groq** key for transcription and a **Gemini** key for Correction, Summary, Action items and Ask. Each has a "Get a free key" link to the provider's key page and a "Test" button.
- Keys go in the existing `SecretStore` (Windows Credential Manager via `keyring`), like the OpenAI and Anthropic keys.
- When the first cloud key is saved, the app shows a one-time notice that Meeting audio or text is sent to that provider, and that Gemini's free tier may use it for training. After that, nothing asks for consent per Meeting for transcription or Correction.

### Engine chain
- Each span of audio (one Source, about 30 s or more, as background spans are today) is transcribed by the first engine that can do it:
  1. **Groq** (`whisper-large-v3`), when a Groq key is set;
  2. **Gemini**, when Groq's daily quota is spent or Groq fails after retries, and a Gemini key is set;
  3. **local Whisper**, as today.
- A per-minute rate limit (HTTP 429 with a short `retry-after`) waits and retries the same engine. A daily-quota 429, an auth error or no network moves on to the next engine. Once an engine's daily quota is spent, it's skipped for the rest of that Processing run.
- A Meeting may mix engines in one Revision. The Transcript records each engine used. If anything fell back, the Meeting view shows a one-line notice, for example "Part of this Meeting was transcribed locally: Groq daily limit reached."
- The Meeting language and the Vocabulary (with the Meeting's Participant names) are passed to every engine: as `language` and `prompt` to Groq, and in the instruction to Gemini.

### Groq engine
- Uses the OpenAI-compatible `audio/transcriptions` endpoint with `response_format=verbose_json` and word plus segment timestamps, so the existing sentence segmenter and Chunk-edge stitching work unchanged.
- Each upload is encoded compactly (16 kHz mono FLAC, or similar) and stays well under Groq's 25 MB free-tier file limit.

### Gemini transcription engine
- Sends one span of about 30 s at a time and asks for structured JSON: sentences with start and end times relative to the span.
- Times are clamped to the span and made monotonic. Each sentence becomes one raw segment with no word timings. ADR 0011 accepts that its Segment times are off by at most one span.

### Cloud transcription during a Recording
- With a Groq key and a network connection, spans are uploaded and transcribed during the Recording on any PC, whatever the GPU or power state. ADR 0010's discrete-GPU and AC-power rule now applies only when the local engine is the one running.
- When the network drops, cloud work pauses and the remainder is handled after Stop by the engine chain. Results are still a cache outside the Library, as in ADR 0010.

### Correction
- After transcription and before the Revision is published, if a Gemini key is set, Processing runs Correction. Gemini gets the Segments (ID and text), the Meeting language, the Vocabulary and the Participant names, and returns a corrected text per Segment ID.
- Correction only changes text. It never changes a Segment's time range, Source or Speaker label, and never adds, removes, merges or splits Segments. Any Segment the result leaves out, duplicates or returns empty keeps its original text.
- Use one request per Meeting where it fits, so the small free daily request limit is enough.
- If Correction fails, times out or hits a quota, the uncorrected text is published and the Meeting view says so.
- The Draft shows uncorrected text, and the Revision shows the corrected text.

### Gemini for Summary, Action items and Ask
- Gemini becomes a fourth `ProviderKind`, a cloud provider alongside OpenAI and Anthropic. Summary, Action items and Ask keep their existing per-Meeting cloud disclosure flow (ADR 0002 is unchanged for them).

### Diagnostics
- Copy diagnostics reports, per Processing run: which engines ran and how much audio each did, fallback reasons, Correction outcome, and cloud round-trip time per span. Keys never appear in logs or diagnostics.

### Measure
- The benchmark script compares local turbo `q5_0`, Groq `whisper-large-v3` and Gemini on the English and Vietnamese FLEURS clips, with and without Correction. It reports WER and wall time, and the results decide whether Gemini becomes the Vietnamese engine (ADR 0011).

### Public copy
- The website privacy page, home page and README no longer say audio never leaves the PC. They say transcription is local by default and moves to Groq or Gemini when the user adds a key.

## Out of scope

- An app-paid relay, a shared or bundled provider key, or rotating several free accounts.
- Paid-tier handling for Gemini, including the EEA, Swiss and UK paid-only rule. The notice mentions it, and nothing else does.
- Automatic Summary after Processing.
- Re-transcribing existing Meetings automatically. The existing "Re-transcribe" action uses the new engine chain.
- A live transcript during a Recording.

## Tickets

1. `01-groq-and-gemini-keys-in-settings.md`
2. `02-groq-transcription-engine.md`
3. `03-engine-chain-with-local-fallback.md`
4. `04-gemini-text-provider.md`
5. `05-gemini-transcription-fallback.md`
6. `06-cloud-transcription-during-recording.md`
7. `07-correction-before-publishing.md`
8. `08-benchmark-cloud-engines.md` (human: needs the user's own keys)
9. `09-update-public-privacy-copy.md`
