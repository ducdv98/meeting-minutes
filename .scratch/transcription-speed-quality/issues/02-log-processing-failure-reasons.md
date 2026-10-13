# Log why Processing failed

Status: ready-for-human
Spec: ../spec.md

## What

A 92-minute run ended in a bare `processing_failed`. Make the next failure explain itself:

- `processing_failed` records:
  - the stage (downloading, preparing, transcribing, publishing);
  - the error kind (the `ProcessingError` variant, plus the engine message trimmed to one line with no file paths or Transcript text);
  - the backend and device;
  - the audio transcribed so far against the total, and the elapsed Whisper time.
- Cancellation is logged as `processing_cancelled`, not as a failure.
- Copy diagnostics shows the last failure line.
- Processing keeps running while a new Recording is made. Do not add pausing.

## Acceptance criteria

- [ ] Diagnostics tests cover the new fields, and show that no Transcript text or user path can reach the log.
- [ ] A forced engine error in a core test produces the expected stage and kind.
- [ ] A cancelled job logs `processing_cancelled`.
- Laptop check: Cancel a running transcription and force a failure (for example delete the model file mid-download): the log shows `processing_cancelled` and a `processing_failed` line with stage, kind, device and audio progress, and Copy diagnostics shows the last failure.
