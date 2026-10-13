# Transcript view, edits and re-transcribe

Status: resolved
Blocked by: 09
Spec: ../spec.md

## What

Show the Transcript beside the player: click a Segment to play it; current Segment highlights during playback. Edit a Segment's text in place (keeps ID and Revision). Re-transcribe with another model or language creates a new Revision and makes it current; old Revisions are kept. Vocabulary editor in settings.

## Acceptance criteria

- [x] Clicking a Segment plays its span.
- [x] Edits persist to the current Revision's file and to `transcript.md`.
- [x] Re-transcribe creates `transcript.r2.json`; earlier Citations still resolve against r1.

## Comments

### 2026-10-12: implemented; laptop check pending

Code is done and tested on Linux. The design is in `../10-core-design.md`.

- Core (`crates/core`):
  - `Library::edit_segment(id, revision, segment_id, text)` changes one Segment's text in the current Revision and keeps its ID, range, speaker and Revision number. The text is trimmed and whitespace is collapsed. The file is written to a temp file, synced, then renamed over `transcript.r<N>.json`, and `transcript.md` is rebuilt. It refuses an older Revision (`StaleRevision`), an unknown Segment and empty text.
  - `Library::transcript_revision(id, n)` and `Transcript::segment(id)` read any Revision, so a Citation stored against r1 still resolves after r2 exists (tested).
  - Each Processing job can carry its own model (`enqueue_with_model`). Without one, it uses the Settings model.
- App:
  - The Meeting view shows the player and the Transcript side by side; they stack below 800 px. Each Segment row shows `[time] Speaker`; clicking it plays the Segment's span. The Segment under the playback position is highlighted and scrolled into view.
  - Each Segment has inline Edit (Ctrl+Enter saves, Escape cancels). Edits are refused while the Meeting is processing, recording or saving. The processor lock is held through the edit, so no new Revision can be written halfway through. An editor left open closes when a new Revision loads.
  - Re-transcribe has a Model picker (the Settings model or any listed model) and uses the existing Language picker. It adds a Revision and keeps earlier ones. The header shows the revision, model, segment count and language.
  - The Vocabulary editor in Settings was already added in ticket 09.
- Verified here: core tests and clippy, the TypeScript typecheck and Vitest pass. `main.rs` passes `cargo clippy --target x86_64-pc-windows-gnu -D warnings` against a stub of the whisper crate.
- Known gaps:
  - If writing `transcript.md` fails after the JSON is saved, the edit is kept but reported as failed. The next edit or rename rebuilds `transcript.md`.
  - Old Revisions cannot be browsed in the UI. Ask and Summary (tickets 13 and 14) will read them through `transcript_revision`.
- Manual check on the laptop:
  1. Open a transcribed Meeting and click a few Segments. Each should play from about 0.5 s before it starts and stop at its end. During normal playback, the highlight should follow the audio.
  2. Edit a Segment and save. Check that `transcript.r1.json` and `transcript.md` show the new text and the Segment ID is unchanged.
  3. Re-transcribe with another model. `transcript.r2.json` should appear, `transcript.r1.json` should be unchanged, and the header should show revision 2 with the new model.

### 2026-10-13: laptop check passed

The owner ran the manual checks on the laptop and all passed. Resolved.
