# Tickets 15 + 16 design: Library search and Delete a Meeting

Tickets: .scratch/windows-mvp/issues/15-library-search.md, 16-delete-meeting.md. Spec: ../spec.md ("Library", "Library search").
Vocabulary: CONTEXT.md (Library search is NOT an Ask: it never generates text). Earlier designs: 05–12-core-design.md.
Existing code: crates/core/src/{index.rs, library.rs}, src-tauri/src/main.rs, src/{App.tsx, MeetingView.tsx, library.ts,
TranscriptView.tsx, timeline.ts}.

Do NOT touch `crates/core/src/lib.rs` module list beyond what is stated, `crates/core/src/ai/*`, or add summary/ask code: tickets 13/14
are built in parallel.

## Ticket 15: search index (index.rs)

rusqlite `bundled` already compiles SQLite with FTS5 (libsqlite3-sys sets `-DSQLITE_ENABLE_FTS5`).

- `SCHEMA_VERSION` 2 → 3. The fresh branch drops and recreates both tables, so an old index is rebuilt by the existing rescan-on-fresh.
  ```sql
  DROP TABLE IF EXISTS meetings; DROP TABLE IF EXISTS segments;
  CREATE TABLE meetings (...unchanged...);
  CREATE VIRTUAL TABLE segments USING fts5(
      text,
      meeting_id UNINDEXED, revision UNINDEXED, segment_id UNINDEXED, start_ms UNINDEXED, end_ms UNINDEXED,
      tokenize = 'unicode61 remove_diacritics 2'
  );
  PRAGMA user_version = 3;
  ```
  `remove_diacritics 2` lets "nguoi" find "người" (Vietnamese typed without accents). Keep the `prepare(... LIMIT 0)` sanity check and
  add one for `segments`.
- Only the **current** Revision (highest `transcript.rN.json`) of each Meeting is indexed.
- New `pub(crate)` methods:
  - `set_transcript(&mut self, meeting_id, Option<&Transcript>)`: in one transaction delete all rows of the Meeting, then insert one row
    per Segment (when Some).
  - `remove_meeting(&mut self, meeting_id)`: in one transaction delete the `meetings` row and its `segments` rows.
  - `rename` also moves `segments` rows (`UPDATE segments SET meeting_id = ?new WHERE meeting_id = ?old`) inside its transaction.
  - `replace(&mut self, meetings: &[(MeetingSummary, i64, Option<Transcript>)])` rebuilds both tables in one transaction (signature
    change; update the only caller, `rescan`).
  - `search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, LibraryError>`.
- Query building (`fn fts_query(input) -> Option<String>`, unit-testable via search behaviour): split the input on anything that is not
  `char::is_alphanumeric`, drop empty pieces, wrap each as an FTS5 string with prefix match `"piece"*` (pieces contain no quotes by
  construction), join with spaces (= AND). No pieces → return `Ok(vec![])` without querying. User input is never interpolated raw, so
  `"`, `*`, `NEAR`, `-`, `:` etc. cannot cause a syntax error.
- SQL: `SELECT s.meeting_id, m.title, m.started_at, s.segment_id, s.start_ms, s.end_ms,
  snippet(segments, 0, char(2), char(3), '…', 12) FROM segments s JOIN meetings m ON m.id = s.meeting_id
  WHERE segments MATCH ?1 ORDER BY rank, m.started_at_ms DESC, s.start_ms LIMIT ?2`.
  (Use the FTS table name in MATCH; alias works in the JOIN.) `\u{2}`/`\u{3}` mark the matched words; the UI turns them into `<mark>`.

```rust
// meeting_file.rs (re-exported from lib.rs next to MeetingSummary: `pub use meeting_file::{MeetingDetail, MeetingSummary, SearchHit};`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchHit {
    pub meeting_id: String, pub title: String, pub started_at: String,
    pub segment_id: String, pub start_ms: u64, pub end_ms: u64,
    pub snippet: String,   // segment text excerpt, matched words wrapped in \u{2} … \u{3}
}
```

## Ticket 15: keeping it in sync (library.rs)

- `rescan`: for each valid folder also read its current Transcript (`highest_transcript_revision` + `read_transcript_revision`); a
  transcript that fails to read/parse is indexed as None (the Meeting stays listed; not a malformed folder).
- `edit_segment` becomes `&mut self` and calls `self.index.set_transcript(id, Some(&transcript))` after the files are written; an index
  error is returned (`LibraryError::Index`) — the file edit itself has already succeeded, so the frontend just shows the error; rescan
  fixes it.
- New `pub fn refresh_transcript(&mut self, id: &str) -> Result<(), LibraryError>` — re-reads the current Transcript of a Meeting and
  calls `set_transcript`. main.rs calls it after `processing::transcribe_meeting` succeeds (a new Revision) — inside the worker, lock
  the Library briefly; ignore its error (rescan fixes it).
- `rename_meeting`: the index rename now carries the Segments (above).
- `record_finalized`, `create_meeting`, import publish: unchanged (no transcript yet).
- `pub fn search(&self, query: &str) -> Result<Vec<SearchHit>, LibraryError>` → `index.search(query, 50)`.

## Ticket 16: delete (library.rs + recycle bin)

```rust
/// Moves a folder to the Windows Recycle Bin (restorable). Non-Windows: Err(io::ErrorKind::Unsupported, "The Recycle Bin is only
/// available on Windows").
pub fn move_to_recycle_bin(path: &Path) -> io::Result<()>;      // new file crates/core/src/recycle_bin.rs, `pub mod recycle_bin;`
```
Windows implementation: `SHFileOperationW` with `wFunc = FO_DELETE`, `fFlags = FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI |
FOF_SILENT`, `pFrom` = absolute path as UTF-16 with a **double** NUL terminator (no `\\?\` prefix — the shell rejects it). Non-zero
return or `fAnyOperationsAborted` → `io::Error::other(format!("Could not move the Meeting to the Recycle Bin (code {code})"))`.
Add `"Win32_UI_Shell"` and `"Win32_Foundation"` (if required by the type) to the core crate's `windows` features.

```rust
impl Library {
    /// Refuses a Meeting with unsaved Recordings (`.chunks` present) → UnsavedRecording. Calls `trash(folder)`; on success removes the
    /// Meeting from both index tables (index error ignored: the folder is gone, the next rescan agrees). On trash error returns it
    /// (LibraryError::Io) and keeps the index untouched.
    pub fn delete_meeting(&mut self, id: &str, trash: &dyn Fn(&Path) -> io::Result<()>) -> Result<(), LibraryError>;
}
```
The trash function is a parameter so tests pass a fake (e.g. one that renames the folder into a temp "bin" dir, or one that fails).

## Tauri (main.rs)

- `search_library(query: String) -> Vec<SearchHit>`.
- `delete_meeting(meeting_id) -> ()`: `meeting_busy(&app, &meeting_id)?` first (refuses the Meeting that is recording or saving, and one
  being processed), plus refuse when this Meeting is the Recorder's current meeting in any phase other than Idle (covered by
  `meeting_busy`, verify) with "Stop the Recording before deleting this Meeting". Then
  `library.delete_meeting(&id, &meeting_minutes_core::recycle_bin::move_to_recycle_bin)`, then `recorder.meeting_deleted(&id)` if the
  Recorder keeps any per-Meeting state such as a "continue this Meeting" offer (check `dismiss_continue`/`continue_meeting` state;
  drop the offer for that id), dismiss any failed processing job for that id, emit `library-changed`, `publish(&app)`.
- `edit_segment` command: `library.as_mut()` (method is now `&mut self`).
- Processing worker: after a successful `transcribe_meeting`, `library.refresh_transcript(&id)`.
- Register both new commands in `generate_handler!`.
- Run the Windows-gnu clippy check for main.rs (stub whisper; see the repo memory/commit history): `main.rs` cannot build on Linux
  otherwise.

## Frontend

- `src/library.ts`: `SearchHit` type, `searchLibrary(query)` (`search_library`), `deleteMeeting(id)` (`delete_meeting`, `{meetingId}`),
  and `snippetParts(snippet): { text: string; match: boolean }[]` splitting on `\u0002`/`\u0003` (pure, unit-tested).
- `src/LibrarySearch.tsx` (new), rendered in the Library list view above the meetings list:
  - `<input type="search" aria-label="Search transcripts">`; searches 250 ms after typing stops (and on Enter); empty → no results shown.
  - Results `<ul aria-label="Search results">`, each a button: Meeting title, start date (same formatting as the list), Segment time
    `formatElapsed(start_ms)`, and the snippet with matches in `<mark>` (React elements, never `dangerouslySetInnerHTML`).
  - No hits for a non-empty query → "No matches". Error → `role="alert"`.
  - Click → `onOpen(hit)`.
- `App.tsx`: keeps `openAt: { segment_id, start_ms, end_ms } | null`; a search result sets `selectedId = hit.meeting_id` and `openAt`.
  `MeetingView` gets an optional prop `openAt`. When the player for that Meeting becomes available (and the transcript has loaded),
  MeetingView calls `player.playSpan(openAt.start_ms, openAt.end_ms)` once, and the Segment row scrolls into view (TranscriptView
  already scrolls the current row when position changes; a playSpan seeks, so seeking sets positionMs — make sure `positionMs` is set
  to `start_ms` immediately so the row highlights even before `timeupdate`). Then App clears `openAt` via `onOpened()`.
- Delete:
  - `src/ConfirmDelete.tsx`: `({ title, onConfirm, onCancel, busy })` → `role="dialog"` `aria-label="Delete <title>?"`, text "The
    Meeting folder moves to the Recycle Bin. You can restore it from there and Rescan to bring it back.", buttons "Delete" (disabled
    while busy) / "Cancel".
  - Library list: a "Delete" button per row (`aria-label="Delete <title>"`) → dialog → `deleteMeeting` → refresh list (the
    `library-changed` event already rescans; also bump `scan`). Error → `role="alert"` in the list view.
  - MeetingView: "Delete" button next to Rename → dialog → before invoking, release the player exactly as `saveRename` does (Windows
    cannot move a folder whose audio is open), then `deleteMeeting`; success → `onDeleted()` prop (App: `setSelectedId(null)`, rescan);
    failure → error alert and reopen the player (`setAudioOpen(true)`).
  - Hide/disable Delete for the Meeting that is currently recording: the backend is authoritative; the UI shows the backend error.
    (Optional nicety: disable in MeetingView when `job` is active.)

## Tests

Core `crates/core/tests/library.rs` (extend; reuse its helpers for making Meetings and writing transcripts):
- search: a word in two Meetings → hits from both with snippet containing `\u{2}word\u{3}`; prefix ("budg" finds "budget");
  diacritics-insensitive ("nguoi" finds "người"); punctuation-only / quotes / `NEAR(` input does not error; empty → empty.
- only the current Revision is searched: r1 has "alpha", r2 has "beta" → "alpha" has no hit after r2 exists and `refresh_transcript`.
- `edit_segment` updates the index (old word gone, new word found).
- rename keeps hits under the new id.
- index rebuild: delete the index file / open with a fresh path → `Library::open` rescans and search works; `rescan` restores rows.
- delete: fake trash moves the folder away → not in `list_meetings`, no search hits, folder gone from root; fake trash moves it back
  + `rescan` → listed and searchable again (the restore acceptance criterion, minus the real Recycle Bin); a Meeting with a `.chunks`
  folder → UnsavedRecording and still present; trash error → error and still listed; unknown id → MeetingNotFound.
- an index created at schema version 2 is upgraded (open with a v2 db file → fresh → rescan).
Frontend (vitest, mocking `@tauri-apps/api/core` like the existing tests):
- `snippetParts`; LibrarySearch debounced call + results + "No matches" + click → onOpen; App: clicking a result opens MeetingView for
  that Meeting; MeetingView with `openAt` calls `playSpan(start, end)` once the player exists (follow how existing MeetingView tests
  fake the player); delete from the list asks for confirmation and calls `delete_meeting`; Cancel does not; error shown; delete from
  MeetingView calls onDeleted.

Manual (laptop, after commit): search a word spoken in two Meetings; click a hit → it plays; delete a Meeting → it is in the Recycle
Bin; restore → Rescan → back and searchable; try to delete the Meeting that is recording → refused.
