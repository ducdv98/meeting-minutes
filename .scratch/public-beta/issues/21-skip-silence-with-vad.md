# Skip silence with Silero VAD

Status: ready-for-human
Blocked by: 17
Spec: ../spec.md

## What

Turn on whisper.cpp's built-in Silero VAD (ADR 0008), so Whisper does no work on silence and makes up less text during quiet stretches.

- whisper-rs 0.16: `FullParams::set_vad_model_path(...)` and `enable_vad(true)`. `enable_vad` panics without a path, so set the path first. Keep the default `WhisperVadParams` unless testing shows clipped words; if so, raise `speech_pad`.
- Ship or download `ggml-silero-v6.2.0.bin` (about 1 MB) with a SHA-256 check, like the Whisper models.
- whisper.cpp 1.8.3 maps segment timestamps back to the original audio. Verify that Segment ranges still line up on the Timeline.
- If the VAD model is missing or fails to load, transcribe without VAD and log it.

## Acceptance criteria

- [ ] A Meeting with long silent stretches transcribes faster, and the ticket 17 log shows audio-after-VAD less than audio duration.
- [ ] Segment start/end times still match the audio when played back (spot-check three Segments after a silence on the laptop).
- [ ] Silence produces no invented text (for example "Thank you." during a 2-minute pause).
- [ ] Transcription still works when the VAD model is unavailable.

## Comments

### 2026-10-13: implemented; laptop check pending

- Silero v6.2.0 downloads during Processing with the LFS SHA-256 and size verified. It stays out of the Settings model list.
- The Whisper adapter uses native Silero VAD and passes only detected speech to Whisper. whisper-rs 0.16 calls `whisper_full_with_state`, which skips whisper.cpp's automatic VAD branch, so the adapter builds the speech buffer and remaps timestamps to the original audio timeline.
- Missing or unloadable VAD falls back to ordinary transcription and logs `vad_unavailable`. The completed event reports the duration sent to Whisper after VAD.
- Core tests, Whisper tests, real-model two-minute silence, and missing-VAD fallback passed. Three playback-aligned Segments and speed on the target Windows laptop remain to be checked.
- Review fixes (Claude):
  - Copied overlap audio now maps to its real time.
  - A time inside the inserted 100 ms gap snaps to the nearest speech edge, instead of being stretched across the removed pause.
  - Language detection uses the first speech VAD finds in the first 5 minutes.
  - A VAD download error after cancelling counts as Cancelled, not `vad_unavailable`.
