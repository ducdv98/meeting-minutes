# Ticket 08 design: Meeting view with Timeline player

Ticket: .scratch/windows-mvp/issues/08-meeting-view-and-player.md. Vocabulary: CONTEXT.md. Earlier designs: 05/06/07-core-design.md.
Existing code: crates/core/src/{library.rs, index.rs, meeting_file.rs, naming.rs, recorder.rs, recording.rs}, src-tauri/src/main.rs,
src/{App.tsx, library.ts, recording.ts}.

## Behaviour

- Library list: each Meeting title is a button that opens the Meeting view. The Meeting view has a "Back to Library" button. No router.
- Meeting view shows: title (h1), date (`<time>` like the list), duration (`formatElapsed`), language ("English" for `en`, "Vietnamese"
  for `vi`, "Not set" for null, otherwise the raw code), and the player when the Meeting has audio ("No audio yet" otherwise).
- Rename: a "Rename" button swaps the h1 for a text input (label "Title") with Save / Cancel. Save is disabled while the trimmed
  input is empty. Enter saves, Escape cancels. On success the view shows the new title and follows the new id. On failure show the
  error in `role="alert"` and keep the input open.
- Rename renames the folder to `folder_name(started_at, new_title)` and the title in meeting.json, then updates the index.

## Core: `Library`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MeetingDetail {          // in meeting_file.rs, re-exported from lib.rs
    pub id: String,
    pub title: String,
    pub started_at: String,
    pub duration_ms: u64,
    pub language: Option<String>,
    /// Absolute path of the mixed MP3 when meeting.json names one and the file exists.
    pub audio_path: Option<PathBuf>,
    pub markers: Vec<Marker>,       // Timeline order, as stored
}

impl Library {
    pub fn meeting(&self, id: &str) -> Result<MeetingDetail, LibraryError>;
    pub fn rename_meeting(&mut self, id: &str, title: &str) -> Result<MeetingSummary, LibraryError>;
}
```

New `LibraryError` variants: `EmptyTitle` ("A Meeting title cannot be empty"), `MeetingNotFound` ("This Meeting is no longer in the
Library"). `meeting`/`rename_meeting` return `MeetingNotFound` when `<root>/<id>/meeting.json` does not exist, and reject an `id`
that is empty, `.`/`..`, starts with `.`, or contains `/` or `\` with `MeetingNotFound` (ids come from the window).

`rename_meeting(id, title)`:
1. `title = title.trim()`; empty → `EmptyTitle`.
2. Refuse with `UnsavedRecording` when the Meeting has unsaved Chunks (`unsaved_recordings`): a Recording is running, saving, or failed
   to save, and its files are written by path.
3. Pick the new folder: `base = folder_name(started_at, title)` (started_at from meeting.json), candidates `base`, `base (2)`, …
   `base (1000)` like `create_meeting`. For each candidate: if it equals `id` → keep the folder (no rename). If it equals `id`
   ignoring case → rename to it (case-only rename; Windows treats it as the same folder). Else if `root/candidate` exists
   (`symlink_metadata` ok) → next candidate. Else `fs::rename(root/id, root/candidate)`; an `AlreadyExists`/`DirectoryNotEmpty`
   error (lost race) → next candidate. Never rename onto an existing path. Exhausted → `CollisionLimit`.
4. Update the title in `<new folder>/meeting.json` as a `serde_json::Value` (set `"title"` only, so unknown fields survive): write
   `meeting.json.tmp` (create+truncate; it is our temp file), `sync_all`, rename over `meeting.json`. If this step fails and the
   folder was renamed, rename it back to `id` (best effort) and return the error.
5. Index: one transaction deleting the old id row and inserting the new summary (`Index::rename(old_id, &summary, started_at_ms)`).
   Return the new `MeetingSummary` (from `read_meeting` of the new folder).

## Recorder

`Recorder::meeting_renamed(&mut self, old_id: &str, new_id: &str)`: replaces `old_id` in `continue_offers` (so "Continue" still works).
Active Recordings and saves never see a rename because step 2 refuses those Meetings.

## Tauri (src-tauri/src/main.rs)

- `meeting_detail(meeting_id) -> MeetingDetail`.
- `rename_meeting(meeting_id, title) -> MeetingSummary`: lock the recorder, then the Library (same order as `start_with`, so no
  Recording can start mid-rename); rename; `recorder.meeting_renamed`; drop locks; emit `library-changed`; `publish`.
- Audio is served through Tauri's asset protocol: `tauri` feature `protocol-asset`, and in tauri.conf.json
  `app.security.assetProtocol = { "enable": true, "scope": ["$DOCUMENT/Meeting Minutes/**"] }`. The frontend uses `convertFileSrc`.

## Frontend

`src/library.ts`: `MeetingDetail` type (`audio_path: string | null`, `markers: Marker[]`), `Marker` union
(`{kind:"pause",at_ms,paused_ms} | {kind:"gap",at_ms} | {kind:"device_switch",at_ms,source,device}`), `meetingDetail(id)`,
`renameMeeting(id, title)`.

`src/player.ts` (pure, unit-tested; the seek API other views use):

```ts
export const LEAD_IN_MS = 500;
export interface TimelinePlayer {
  seek(ms: number): void;                       // sets position, keeps play/pause state, cancels any span
  play(): void; pause(): void;
  playSpan(startMs: number, endMs: number): void; // seek to max(0, startMs - LEAD_IN_MS), play, pause once position >= endMs
}
export function createTimelinePlayer(media: HTMLMediaElement): TimelinePlayer & { dispose(): void };
export function markerLabel(marker: Marker): string;          // "Paused for 2m 05s" / "Gap between Recordings" / "Microphone switched to X" / "System audio switched to X"
export function markerPosition(atMs: number, durationMs: number): number;  // percent, clamped 0..100, 0 when duration is 0
```

Span end is checked on `timeupdate`; a manual `seek`, `play`, or `playSpan` replaces/cancels the pending span. Positions are Timeline ms
(the MP3 lines up with the Timeline, see the LAME tag commit). `playSpan` with `endMs <= startMs` plays nothing past the lead-in
(pauses at the first timeupdate). Clamp `endMs` to nothing; the media just ends.

`src/Player.tsx`: renders `<audio preload="metadata" src={convertFileSrc(audio_path)}>` (no native controls), a Play/Pause button, a
current-time / duration readout (`formatElapsed`), and a scrubber: an `<input type="range" aria-label="Position" min=0
max=duration_ms step=1000>` with a marker layer over it. Each Marker is a `<button>` positioned at `left: markerPosition(...)%` with
`aria-label`/`title` = `markerLabel` plus the time, and `data-kind`; clicking it seeks there. The scrubber max is the Meeting's
`duration_ms` (Timeline), not `audio.duration`.
`PlayerContext` (React context exposing the `TimelinePlayer` or null) and `usePlayer()` hook, provided by `MeetingView` so later views
(transcript, citations) call `usePlayer()?.playSpan(start_ms, end_ms)`.

`src/MeetingView.tsx`: loads `meetingDetail(id)`, renders the fields, Rename, Back, and `Player`. Before calling `renameMeeting` it
pauses the audio and drops its source (Windows cannot rename a folder whose file is open), then reloads the detail by the new id.
`App.tsx` holds `selectedId: string | null`; the Meeting view's `onRenamed(newId)` updates it.

## Tests

- tests/library.rs: rename renames the folder, keeps other files and unknown meeting.json fields, updates the index (old id gone,
  new id listed with the new title); rename to a taken name gets ` (2)` and never touches the other folder; rename to the same title
  keeps the folder; empty title → `EmptyTitle`; Meeting with `.chunks/recording-1` → `UnsavedRecording` and nothing changes; unknown
  id and `..` → `MeetingNotFound`; `meeting(id)` returns markers, language, and `audio_path` only when the file exists.
- recorder test: `meeting_renamed` moves a continue offer.
- src/player.test.ts with a fake media element: `playSpan` seeks to start − 500 ms (clamped at 0), plays, pauses at end on timeupdate;
  a manual seek cancels the span; `markerPosition` / `markerLabel`.
- src/MeetingView.test.tsx: shows title/date/duration/language, marker buttons with labels, rename success follows new id, rename
  error shows alert. App.test.tsx: clicking a title opens the view; Back returns.
