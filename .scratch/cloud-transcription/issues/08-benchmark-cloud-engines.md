# Benchmark the cloud engines

Status: ready-for-human
Spec: ../spec.md
Blocked by: 02, 05, 07

## What

- Extend `scripts/benchmark-transcription` to run, on the English and Vietnamese FLEURS clips:
  - local turbo `q5_0`;
  - Groq `whisper-large-v3`;
  - Gemini transcription;
  - each of the above with Correction.
- Read keys from `GROQ_API_KEY` and `GEMINI_API_KEY`, and skip an engine when its key is missing.
- Report WER and wall time per engine, per language. Stay well within the free tiers.
- Human: run it with your own free keys and paste the table into `## Comments`. Then decide whether Gemini becomes the Vietnamese engine (ADR 0011), and whether Correction is worth keeping on by default.

## Acceptance criteria

- [ ] The script runs end to end with both keys, with one key, and with none.
- [ ] Results table recorded below.
