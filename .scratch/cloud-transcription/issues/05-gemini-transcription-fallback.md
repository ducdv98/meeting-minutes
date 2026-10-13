# Gemini transcription fallback

Status: ready-for-agent
Spec: ../spec.md
Blocked by: 03, 04

## What

- Add a Gemini `SpeechEngine` and put it second in the chain from ticket 03. It's used when Groq is exhausted or failing and a Gemini key is set.
- For each span of about 30 s:
  - send the audio inline (FLAC or WAV, base64) with an instruction to transcribe it verbatim in the Meeting language, using the Vocabulary and Participant names for spelling;
  - ask for JSON `{ sentences: [{ start_s, end_s, text }] }` relative to the span, using the shared `generate_json` / schema support from ticket 04.
- Clamp times to the span, make them monotonic, and drop empty sentences. Each sentence becomes one `RawSegment` with no `words`; the segmenter must accept that.
- If the output is invalid JSON or claims more audio than the span holds, treat it as `Other` and let the chain move on.
- Spans longer than about 30 s, as during background stitching, are split into pieces of 30 s or less before sending.

## Acceptance criteria

- [ ] Fake-HTTP tests: mapping, clamping, monotonic repair, invalid-output fallback.
- [ ] Chain test: Groq quota hit, then Gemini, then a Gemini quota hit, then local, all in one Revision.
- [ ] The segmenter handles segments without word timings (unit test).
