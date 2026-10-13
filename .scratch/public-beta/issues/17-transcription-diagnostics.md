# Report transcription speed in diagnostics

Status: ready-for-agent
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
