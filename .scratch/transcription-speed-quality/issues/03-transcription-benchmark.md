# Transcription benchmark

Status: ready-for-agent
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
