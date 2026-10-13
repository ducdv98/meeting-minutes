# Meeting view with Timeline player

Status: resolved
Blocked by: 05
Spec: ../spec.md

## What

Meeting detail screen: title (renameable, which renames the folder), date, duration, language, a player for the mixed MP3 with Markers shown on the scrubber, and a seek API other views will use (seek to an ms position and play a span).

## Acceptance criteria

- [x] Renaming a Meeting renames its folder safely and updates the index.
- [x] Markers (pause, gap, device switch) show on the scrubber.
- [x] `playSpan(start_ms, end_ms)` plays that span with a small lead-in.

## Comments

### 2026-10-12: implemented; laptop check pending

Code is done and tested on Linux. The Windows build passes `cargo clippy --target x86_64-pc-windows-gnu`. The design is in `../08-core-design.md`.

- Core (`crates/core`):
  - `Library::meeting(id)` returns `MeetingDetail`: title, date, duration, language, markers, and `audio_path` (only when the MP3 exists).
  - `Library::rename_meeting(id, title)` renames the folder to `<YYYY-MM-DD HHmm> <title>`, adding ` (2)`, ` (3)`… when that name is taken. It never renames onto an existing path. A case-only rename stays in the same folder.
  - It changes only `"title"` in `meeting.json`, so unknown fields survive. If that write fails, the folder is renamed back.
  - The index row moves in one transaction. If that fails, the next rescan fixes it.
  - Renaming is refused while the Meeting has unsaved Chunks, and in the app while the recorder still has it active or saving.
  - A Continue offer follows the rename.
- App:
  - Meeting titles in the Library open the Meeting view. It shows title, date, duration, language, and the player.
  - Rename uses an inline input. Before renaming, the view unloads and unmounts the audio, because Windows cannot rename a folder whose file is open.
  - The player plays `meeting.mp3` through Tauri's asset protocol, scoped to `$DOCUMENT/Meeting Minutes/**`.
  - The scrubber is measured on the Timeline (`duration_ms`). Pause, gap and device-switch Markers are buttons on it with labels; clicking one seeks there.
  - Seek API for later views: `usePlayer()` (from `src/Player.tsx`) gives `seek(ms)`, `play()`, `pause()` and `playSpan(start_ms, end_ms)`. `playSpan` starts 500 ms early (never before 0) and pauses at `end_ms` on `timeupdate`, which can run up to about 250 ms past the end.
- Known gaps:
  - Between the "is this name free" check and `fs::rename`, another process could create an empty folder with the same name. That is not guarded.
  - WebView2 may keep the MP3 open briefly after the player unmounts. If so, rename fails with an error and can be retried.
- Manual check on the laptop:
  1. Open a recorded Meeting with a pause (and ideally a headset switch). Play, scrub, and click each Marker. Playback should jump there.
  2. Rename it while the audio is playing. The folder in `Documents\Meeting Minutes` should be renamed, the title updated, and playback should work again.
  3. Rename it to the title of another Meeting from the same minute. You should get a ` (2)` folder, and the other folder should be untouched.


### 2026-10-13: laptop check passed

The owner ran the manual checks on the laptop and all passed. Resolved.
