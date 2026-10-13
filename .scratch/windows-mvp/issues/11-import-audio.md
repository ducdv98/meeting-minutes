# Import an audio file as a Meeting

Status: resolved
Blocked by: 09
Spec: ../spec.md

## What

Import mp3/m4a/wav/flac (file picker and drag-and-drop) as a new Meeting: copy the audio into a new Meeting folder, then Process it like a recorded one with Speaker label Unknown.

## Acceptance criteria

- [x] Imported Meeting appears in the Library, plays, and gets a Transcript.
- [x] The original file is copied, never moved or modified.

## Comments

### 2026-10-12: implemented; laptop check pending

Code is done and tested on Linux. The design is in `../11-core-design.md`.

- Core (`crates/core`):
  - `import` decodes mp3, m4a (AAC-LC), wav and flac with symphonia 0.6.1, streaming and averaged to mono. It refuses sample rates outside 8–192 kHz on the first packet, so a file is refused at import rather than later during Processing.
  - The Meeting is built in a hidden staging folder in the Library (`.import-<pid>-<nanos>`), which rescan and orphan recovery skip. The original is copied (read only, never moved) to `imported.<ext>` through a synced temp file. The staged copy is then decoded to validate it and measure its duration. Finally the folder is renamed to `<date> <file name>`, adding ` (2)`… on a name collision, and indexed. Any failure or cancel removes the staging folder.
  - `meeting.json` gets `audio = "imported.<ext>"` (played directly; no MP3 is made) and an `import` block with the file, original name and import time. It has no Recordings. Continuing an imported Meeting is refused (`ImportedMeeting`).
  - Processing resamples the file to 16 kHz with an exact polyphase windowed-sinc filter. 10 minutes of 44.1 kHz stereo takes about 1 s in a release build. Speaker labels are always Unknown.
- App:
  - "Import audio" in the Library header opens a file picker (tauri-plugin-dialog 2.8.2, run from Rust; no JS permission needed). Files can also be dropped anywhere on the window, which shows a drop hint while dragging.
  - Imports run one at a time on a background thread, with "Importing <name>… (+N queued)" and a dismissible error list. Unsupported files dropped on the window are listed as errors.
  - When auto-processing is on, the new Meeting is queued for Processing before the Library shows it. The Meeting view shows "Imported audio".
  - Quit cancels a running import.
- Verified here: core tests (including WAV at 8, 22.05, 44.1 and 44.101 kHz, FLAC and MP3 fixtures, tone survival and alias rejection, copy byte-identical with the source unchanged, collisions, cancel and corrupt input leaving no folders, transcription with Unknown labels) and clippy, the TypeScript typecheck and Vitest pass. `main.rs` passes `cargo clippy --target x86_64-pc-windows-gnu -D warnings` against a stub of the whisper crate.
- Known gaps:
  - Like recorded Meetings, the whole 16 kHz buffer is held for whisper (about 230 MB per hour).
  - HE-AAC m4a files (common for some phone recorders) are refused, because symphonia decodes only AAC-LC.
  - If the index insert fails after the folder is published, the Meeting appears after the next rescan.
- Manual check on the laptop:
  1. Import a real m4a (for example a Windows Voice Recorder file) with the button, and an mp3 by dropping it on the window. Both should appear in the Library, play, and get a Transcript with Unknown speakers.
  2. Check that the original files are unchanged (same size and modified time) and still in place.
  3. Drop a non-audio file. You should see an error and no new Meeting.

### 2026-10-13: laptop check passed

The owner ran the manual checks on the laptop and all passed. Resolved.
