# Skip silence with Silero VAD

Status: ready-for-agent
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
