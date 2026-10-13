# Transcription benchmark

Status: ready-for-human
Spec: ../spec.md

## What

Add a repeatable way to measure speed and quality:

- A script, for example `scripts/benchmark-transcription`, that downloads a small set of public clips with reference transcripts into a git-ignored folder:
  - about 10 English and 10 Vietnamese clips from FLEURS or Common Voice;
  - plus one long stitched clip of about 10 minutes, with silences, to exercise VAD and Segment lengths.
- A benchmark runner (an ignored test or a small binary in `crates/whisper`) that, for each model × backend/device given on the command line, reports:
  - WER, computed after normalising case and punctuation (and diacritics kept for Vietnamese);
  - RTF and audio after VAD;
  - Segment duration statistics (median, p10, p90, and the share under 1 s or over 20 s).
- It outputs a Markdown table, so results can be pasted into a ticket.
- It uses the same `WhisperEngine` and `transcribe_meeting` path the app uses, not a separate code path.

## Acceptance criteria

- [ ] The runner works on CPU in CI-free local runs on Linux, with `base` as a smoke test.
- [ ] README or `docs/` explains how to run it on Windows with a chosen device.
- [ ] WER computation has unit tests.
- Laptop check: Run `scripts/benchmark-transcription list-devices`, then `run` with the 780M and RTX 4050 IDs (ticket 04).

## Comments

2026-10-13, CPU smoke run on the Linux dev box (4-core Neoverse-N1, `base`, Silero VAD), before sentence-sized Segments (ticket 07):

| Model | Backend / device | Set | WER | RTF | After VAD | Median s | p10 s | p90 s | <1 s | >20 s |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| base | CPU | English short (10) | 10.3% | 0.467 | 86.3s | 4.5 | 2.9 | 6.7 | 0% | 0% |
| base | CPU | Vietnamese short (10) | 43.6% | 0.338 | 89.0s | 4.1 | 2.1 | 9.7 | 0% | 6% |
| base | CPU | English stitched (1) | 69.7% | 0.433 | 382.0s | 9.5 | 4.5 | 30.7 | 0% | 36% |

Finding: on the 10-minute stitched clip `base` keeps only 387 of 1016 reference words. Whole sentences are skipped and the text then loops back to earlier sentences, so WER is 69.7% while the same clips alone score 10.3%. The stitched clip goes through the same Import → VAD → `transcribe_meeting` path as the app, so long Meetings likely lose speech today. Check whether larger models (ticket 04) show the same, and whether disabling `condition on previous text` or shorter VAD-packed windows fixes it, before trusting long-Meeting WER.
