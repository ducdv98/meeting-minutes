# Record a Meeting end to end

Status: resolved
Blocked by: 03, 04
Spec: ../spec.md

## What

Turn the spike into the real capture module: Record / Pause / Resume / Stop from the window, tray and a global hotkey. Each Source is written as Chunks; Stop finalises one FLAC per Source per Recording plus a mixed MP3 of the Timeline into the Meeting folder, then updates `meeting.json` and the index. Show the Recording notice on the first Recording ever and a visible indicator while recording. Incorporate drift findings from ticket 03.

## Acceptance criteria

- [x] Recording only starts on deliberate action; only one Recording can be active.
- [x] Pause/resume stays in one Recording and adds a pause Marker; the Timeline excludes paused time.
- [x] After Stop: FLAC per Source, mixed MP3, and `meeting.json` with duration, Recordings and Markers.
- [x] Failed finalisation keeps the Chunks and offers Retry save without re-recording.
- [x] Tray icon and window both show elapsed time while recording.
- [x] Manual check on the laptop: a 30-minute call produces a playable Meeting.

## Comments

### 2026-10-12 — implemented; laptop check pending

Code is done and tested on Linux. The Windows build is checked with `cargo clippy --target x86_64-pc-windows-gnu`. Status is `ready-for-human` until the 30-minute laptop check is run.

- Core (`crates/core`):
  - `capture.rs` holds the Source placement on the QPC clock, carried over from the spike, with pause/resume. `capture/wasapi_input.rs` holds the WASAPI inputs and the QPC clock (Windows only, default communications endpoints).
  - `recording.rs` holds the rest of the capture path:
    - Chunks: 10 s raw f32 files in `.chunks/recording-N/`. The Microphone is stored mono. A Chunk in progress is a `.part` file; it is renamed after `sync_all`.
    - `CaptureSession`: one thread per Source.
    - `recording.json` manifest.
    - `finalize_recording`: streams `recording-N microphone.flac`, `recording-N system.flac` and `meeting.mp3`, then rewrites `meeting.json`.
  - `recorder.rs` is the app state: at most one active Recording, the Recording notice gate, and pending or failed saves with Retry save.
  - `settings.rs` stores whether the notice was acknowledged.
  - The index now stores `duration_ms` (index version 2, so it rebuilds).
- Policies are in ADR 0005: drop/pad on QPC instead of resampling, hard clipping, mono Microphone, 16-bit FLAC, 128 kbps stereo MP3 via LAME.
- If finalisation fails, every file that attempt created is removed and the Chunks are kept. Final files are created with `create_new`, so nothing is overwritten.
- App:
  - Record / Pause / Resume / Stop in the window, tray menu and hotkeys `Ctrl+Alt+Shift+R` (record/stop) and `Ctrl+Alt+Shift+P` (pause/resume).
  - The first Recording ever shows the Recording notice. Tray or hotkey open the window to show it.
  - Elapsed time shows in the window indicator, the tray tooltip and a tray menu line. The tray icon gets a red dot while recording.
  - Saving runs in the background. Quit saves an active Recording and waits for running saves.
- Left for other tickets:
  - Failed or pending saves are kept only in memory. After a restart the Chunks stay on disk for ticket 06 recovery.
  - `meeting.mp3` is created once, so a second Recording in the same Meeting (ticket 06) must rebuild it.
  - Default-device following, device-switch Markers and logging of failed reopens are ticket 07. For now the device is reopened only after a read error, and errors are not surfaced.
  - The ticket 03 request for QPC offsets on `idle_pad`/stop events is not done; there is no event log in the app yet.
- Not verified here: the MSVC build of `mp3lame-sys` (Windows CI will show it) and real WASAPI capture.
- Manual check still to do: a 30-minute call on the laptop produces a playable Meeting. Both FLACs and the MP3 should play, and pause/resume should show in `meeting.json`.

### 2026-10-12 — laptop run reviewed; MP3 delay fixed

The user's laptop run (2:26 with one pause) and a real meeting both worked. In the 2:26 run, System audio was silent because YouTube played on another device, which the user says is expected. Both FLACs have identical frame counts that match `duration_ms`, and all three files decode cleanly. The pause left no silent gap and no click.

Fixed: `meeting.mp3` played 47 ms (2,257 samples) behind the Timeline. LAME's encoder delay was not trimmable because no Info/LAME tag was written, and `FlushNoGap` also dropped the final partial frame. The encoder now reserves and writes the LAME tag and flushes with `FlushGap`. ffmpeg decodes the MP3 sample-exact against the FLAC (a click test lands at offset 0 ms, with the same length).

Resolved 2026-10-12: the user tested a real meeting on the laptop, it worked, and they resolved the ticket. No 30-minute run is recorded.
