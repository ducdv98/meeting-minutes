# Rename a Meeting at any time

Status: ready-for-agent
Spec: ../spec.md

## What

Stop moving the Meeting folder on rename (ADR 0009).

- **Folder naming:** `naming::folder_name` drops the Title. New folders, recorded or imported, are `YYYY-MM-DD HHMM`, with ` (n)` on a collision.
- **`rename_meeting`** (`crates/core/src/library.rs`):
  - writes the Title into `meeting.json`, updates the index row, and re-renders `transcript.md` and `summary.md`;
  - no folder move, no ID change, no `UnsavedRecording` refusal.
- **Remove the busy checks for rename:**
  - the backend `meeting_busy` check on rename (`src-tauri/src/main.rs`);
  - the UI `disabled` rules on Rename;
  - the code that closes the player before saving (`App.tsx`, `MeetingView.tsx`);
  - the ID remapping after a rename (`onRenamed`, `Recorder::meeting_renamed`) that is no longer needed.
- **Concurrent writers:** other code also writes `meeting.json` (Recording, Processing, cloud consent, language). Make sure a rename cannot lose another writer's change, and that they cannot lose the rename. Use a read-modify-write under one lock, or an equivalent.
- **Recording screen:** the Title can be edited while recording.

## Acceptance criteria

- [ ] A new recorded Meeting and a new Import get a start-time-only folder name; a same-minute collision gets ` (2)`.
- [ ] Renaming during transcription, Summary generation, an Ask, playback, and an active Recording succeeds immediately. The folder and Meeting ID are unchanged, and playback is not interrupted.
- [ ] The new Title shows in the Library list, the Meeting view, `transcript.md` and `summary.md`.
- [ ] A rename at the same time as another `meeting.json` write keeps both changes (core test).
- [ ] Existing folders with a Title in their name still load and rename correctly.
- [ ] Core and frontend tests updated; Windows clippy check passes.

## Comments

### 2026-10-13: implemented

- New folders are `YYYY-MM-DD HHMM` (+ ` (n)`). Rename writes only `meeting.json`, updates the index row and re-renders the Markdown files; ID and folder never change.
- `crates/core/src/metadata.rs`: every `meeting.json` writer after creation (rename, language, cloud consent, recording finalization) goes through one process-wide lock. Transcript and Summary Markdown are written with the Title read under that lock, so a rename during Processing or Summary isn't overwritten by the old Title.
- Rename is no longer disabled while busy; the player stays open; the Recording screen can edit the Title.
- Checks: core tests, `pnpm typecheck`, Vitest, Windows-gnu clippy of `src-tauri` with a whisper stub.
- Review notes left as is: Markdown write errors on rename are ignored (as before; `meeting.json` is the source of truth); the global lock is held during the small Markdown writes.
