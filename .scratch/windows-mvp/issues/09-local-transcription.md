# Local transcription into Segments

Status: ready-for-human
Blocked by: 06, 08
Spec: ../spec.md

## What

Transcribe a Meeting on-device with whisper.cpp via `whisper-rs`. Model manager: download on first use with progress, checksum and cancel; default multilingual `small`. Per-Meeting language (English, Vietnamese, auto). Vocabulary from settings becomes the initial prompt. Output normalised into Segments with app-assigned IDs and ms ranges on the Timeline, written as `transcript.r1.json` + `transcript.md`. Speaker label from Source activity: Me / Remote / Unknown. Processing starts automatically after Stop unless turned off in settings; progress and cancel visible.

## Acceptance criteria

- [ ] A 60-minute Meeting transcribes on the laptop; the time taken is recorded in the ticket.
- [x] Segments are ordered, non-empty, `0 <= start_ms < end_ms <= duration_ms`, IDs stable across reloads.
- [ ] Speaker labels: headset call shows Me/Remote correctly in a spot check; overlap → Unknown.
- [x] Turning off auto-Processing leaves the Meeting saved with a Transcribe button.
- [x] Unit tests: normalisation, ID assignment, Timeline offsets across Recordings, speaker labelling.

## Comments

### 2026-10-12: implemented; laptop checks pending

Code is done and tested on Linux. The design is in `../09-core-design.md`.

- Core (`crates/core`):
  - Audio preparation streams each Recording's FLACs. It mixes the Microphone and System audio, filters and downsamples from 48 kHz to 16 kHz, and keeps a 20 ms loudness level per Source.
  - Engine output becomes Segments placed on the Timeline. Text is trimmed, empty segments and tags like `[BLANK_AUDIO]` are dropped, ranges are clamped, and IDs `s00001`… are assigned in time order.
  - Speaker label: a Source is active in a Segment when at least 15% of its 20 ms frames (or 500 ms in total) are louder than −40 dBFS. Microphone only → Me, System audio only → Remote, anything else → Unknown.
  - Each run writes `transcript.r<N>.json` as a new Revision: written to a temp file, synced, then hard-linked into place, so an existing Revision is never overwritten. `transcript.md` is rebuilt from the latest Revision, including after a rename.
  - Model manager offers multilingual base / small (default) / medium / large-v3-turbo. Each download is checked against the size and SHA-256 published by Hugging Face (checked on 2026-10-12). It shows progress, can be cancelled, and has 30 s timeouts.
  - The Processing queue runs one job at a time. Continuing a Meeting while it is being transcribed queues it to run again.
- `crates/whisper`: whisper-rs 0.16.0 on CPU, greedy decoding.
  - Auto-detect picks English or Vietnamese, whichever whisper rates higher.
  - Vocabulary is passed as the initial prompt.
  - Works around a whisper-rs 0.16.0 bug: `set_abort_callback_safe` stores the closure under a different type than its callback reads. We pass a `Box<dyn FnMut() -> bool>` so the two types match.
- App:
  - Processing starts automatically after a successful save unless "Transcribe automatically after recording" is off in Settings, in which case the Meeting view has a Transcribe button.
  - Progress and Cancel show in both the Library list and the Meeting view. The Meeting view also has a language picker (Auto-detect / English / Vietnamese) and Re-transcribe.
  - Jobs wait while a Recording is running or saving.
  - Rename and language changes are refused while the Meeting is recording, saving or being processed.
  - Quit cancels the job and waits for it to stop.
- Verified here:
  - A real run with whisper `tiny`: the JFK sample on the Microphone in Recording 1 and on System audio in Recording 2 gave two Segments, `s00001` 0–11000 Me and `s00002` 11000–22000 Remote, with English auto-detected. A second run wrote r2 and left r1 untouched.
  - `main.rs` type-checks for Windows (`cargo clippy --target x86_64-pc-windows-gnu`) against a stub of the whisper crate. whisper.cpp itself does not cross-compile with this machine's MinGW headers, so Windows CI (MSVC) is the first full build.
- Known gaps:
  - A running transcription keeps going if a new Recording starts. It uses at most (cores − 1) threads, capped at 8.
  - No VAD, so whisper may invent text over long silences.
  - Re-transcribe does not ask for confirmation. It only adds a Revision.
  - A job interrupted by Quit is not resumed. Press Transcribe again.
- Manual checks on the laptop:
  1. Record a 60-minute call (or continue a long Meeting), let it transcribe with `small`, and record the time taken here. The first run also downloads the 488 MB model.
  2. With a headset, spot-check that your own lines are Me and the other person's are Remote. Talk over them once and check that the overlapping span is Unknown.
  3. Turn off auto-transcribe in Settings, record a short Meeting, and check it is saved with a Transcribe button.
  4. Cancel once during the model download and once during transcription. Nothing partial should be left, apart from a `.part` file that the next download removes.

