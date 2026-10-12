# Library folders and rebuildable index

Status: resolved
Blocked by: 01
Spec: ../spec.md

## What

Implement the Library: Meeting folder layout and `meeting.json` as in the spec, collision-safe naming, and the SQLite index in app data that can be rebuilt from the folders. The UI lists Meetings by date and title. No audio yet: create Meetings via a dev command or fixture.

## Acceptance criteria

- [x] Meeting folder name `<YYYY-MM-DD HHmm> <title>`; collisions get a suffix; nothing is ever overwritten.
- [x] Deleting the index and restarting rebuilds the same Meeting list from the folders.
- [x] A hand-edited or added Meeting folder is picked up on rescan; malformed folders are reported, not crashing.
- [x] Unit tests cover naming, collision and rebuild.

## Comments

Implemented 2026-10-12.

- Core (`crates/core`): `naming.rs` (folder name + Windows-safe title, max 80 chars, "Untitled meeting" fallback), `meeting_file.rs` (`meeting.json` schema_version 1; `recordings`/`markers` are untyped placeholders for ticket 05), `index.rs` (SQLite in app data, `PRAGMA user_version`; recreated if corrupt or outdated), `library.rs` (`Library::open/create_meeting/list_meetings/rescan`).
- Meeting id = folder name. Folders are created with `create_dir` plus `(2)`, `(3)` suffixes, and `meeting.json` with `create_new`, so nothing is overwritten. `open` rebuilds a new or recreated index; rescan replaces the index from the folders and reports malformed folders (missing or invalid `meeting.json`, wrong schema_version, bad `started_at`). Folders whose names start with `.` are ignored.
- App: Library at `Documents\Meeting Minutes`, index at `<app data>\index.sqlite3`; the UI rescans on load and on a Rescan button, lists date + title newest first, and shows malformed folders.
- Dev command: `cargo run -p meeting_minutes_core --example create_meeting -- <library-root> <index-path> <title> [<rfc3339>]`.
- Not verified here: building `src-tauri` (Linux box lacks GTK libs); Windows CI covers it. Manual check on Windows still worth doing: run the app, create a Meeting with the example, delete the index, restart.
