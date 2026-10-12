# Local transcription into Segments

Status: ready-for-agent
Blocked by: 06, 08
Spec: ../spec.md

## What

Transcribe a Meeting on-device with whisper.cpp via `whisper-rs`. Model manager: download on first use with progress, checksum and cancel; default multilingual `small`. Per-Meeting language (English, Vietnamese, auto). Vocabulary from settings becomes the initial prompt. Output normalised into Segments with app-assigned IDs and ms ranges on the Timeline, written as `transcript.r1.json` + `transcript.md`. Speaker label from Source activity: Me / Remote / Unknown. Processing starts automatically after Stop unless turned off in settings; progress and cancel visible.

## Acceptance criteria

- [ ] A 60-minute Meeting transcribes on the laptop; the time taken is recorded in the ticket.
- [ ] Segments are ordered, non-empty, `0 <= start_ms < end_ms <= duration_ms`, IDs stable across reloads.
- [ ] Speaker labels: headset call shows Me/Remote correctly in a spot check; overlap → Unknown.
- [ ] Turning off auto-Processing leaves the Meeting saved with a Transcribe button.
- [ ] Unit tests: normalisation, ID assignment, Timeline offsets across Recordings, speaker labelling.

## Comments
