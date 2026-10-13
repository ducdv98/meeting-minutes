# Report transcription speed in diagnostics

Status: ready-for-human
Blocked by: 02
Spec: ../spec.md

## What

Make transcription speed measurable on every machine (ADR 0008). After each local Transcription, log one event with:
- model name;
- backend (GPU device name, or `CPU`);
- thread count;
- audio duration, and audio after VAD when VAD is on;
- wall time for audio preparation and for Whisper;
- real-time factor (RTF = Whisper wall time ÷ audio duration).

Copy diagnostics gains:
- the whisper.cpp system info line (compiled SIMD flags, such as `AVX2 = 1`);
- the transcription backend;
- the last transcription's RTF line.

No telemetry: this stays in the local log.

## Acceptance criteria

- [ ] After a Transcription, the log has a `transcription_completed` line with model, backend, threads, durations and RTF.
- [ ] Copy diagnostics shows the whisper.cpp system info, the backend and the last RTF.
- [ ] Meeting content (Transcript text, titles) never appears in the log line.
- [ ] Core unit tests cover the RTF/log formatting.

## Comments

### 2026-10-13: implemented; laptop check pending

Codex implemented this ticket. Claude reviewed it and fixed the review findings.
- After a Transcription, core logs `transcription_completed model=… backend=… threads=… audio_ms=… audio_after_vad_ms=none preparation_ms=… whisper_ms=… rtf=…`.
  - The model must be in the model list, otherwise it logs as `unknown`.
  - The backend must be `CPU`, `Vulkan` or `unknown`.
  - `audio_ms` is the audio actually sent to Whisper.
- `SpeechEngine` gains required `backend()` and `thread_count()` methods.
- Copy diagnostics adds the whisper.cpp system info, the transcription backend and the last RTF.
- Verified: core tests, clippy, and the Windows-gnu clippy check of `main.rs` against the stub whisper crate.
- Laptop check: transcribe a Meeting, then use Copy diagnostics. The system info shows SIMD flags, and the RTF line appears.
