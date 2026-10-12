# Ticket 10 design: Transcript view, edits and re-transcribe

Ticket: .scratch/windows-mvp/issues/10-transcript-view-and-corrections.md. Vocabulary: CONTEXT.md. Previous design: 09-core-design.md
(file formats, `transcript.r<N>.json`, `render_markdown`, Processor). Existing code: crates/core/src/{library.rs, transcript.rs,
processing.rs}, src-tauri/src/main.rs, src/{MeetingView.tsx, Player.tsx, player.ts, processing.ts, SettingsView.tsx}.

Already done by ticket 09 (do not rebuild): the Vocabulary editor in Settings, the Meeting language picker, the Re-transcribe button,
new Revisions never overwriting old ones.

## Core

### transcript.rs

```rust
impl Transcript {
    /// The Segment with this ID in this Revision (how a Citation resolves).
    pub fn segment(&self, segment_id: &str) -> Option<&Segment>;
}
```

### library.rs

```rust
/// The given Revision, or None when `transcript.r<revision>.json` does not exist. Old Revisions stay readable after re-transcribe,
/// so Citations stored as (revision, segment_id) still resolve against them.
pub fn transcript_revision(&self, id: &str, revision: u32) -> Result<Option<Transcript>, LibraryError>;

/// Replaces one Segment's text in the current Revision. Keeps the Segment ID, range, speaker and the Revision number.
/// - `revision` must be the current (highest) Revision, else `LibraryError::StaleRevision` (nothing written).
/// - unknown `segment_id` → `SegmentNotFound`.
/// - text is trimmed and inner whitespace collapsed (same rule as normalisation); empty → `EmptySegmentText`.
/// - writes `transcript.r<N>.json.edit.tmp` (pretty JSON + trailing newline, synced), then renames it over `transcript.r<N>.json`;
///   the tmp is removed on failure. Then rewrites `transcript.md` from that Revision with the Meeting title
///   (existing `write_transcript_markdown` + `render_markdown`).
/// Returns the updated Transcript.
pub fn edit_segment(&self, id: &str, revision: u32, segment_id: &str, text: &str) -> Result<Transcript, LibraryError>;
```

Refactor `transcript()` to share a "highest revision number in folder" helper with `edit_segment`.

New `LibraryError` variants (Display):
- `StaleRevision` → "The Transcript has a newer Revision; reload it before editing"
- `SegmentNotFound` → "This Segment is not in the Transcript"
- `EmptySegmentText` → "A Segment's text cannot be empty"

### processing.rs — per-job model for re-transcribe

- `ProcessingJob` gains `pub model: Option<String>` (serialized; None = model from Settings).
- `Processor::enqueue_with_model(&mut self, meeting_id: &str, model: Option<&str>) -> bool`; `enqueue(id)` calls it with None.
  Same transitions as `enqueue`; additionally sets the job's `model` to the given value whenever it returns true or the job is the
  running one getting a rerun (so the rerun uses the newest choice). A Queued job that is enqueued again with a different model
  updates its `model` (returns false as before).
- `Processor::job_model(&self, meeting_id: &str) -> Option<String>`.

## Tauri (main.rs)

- `transcribe_meeting(meeting_id, model: Option<String>)`: when Some, must be a name in `MODELS` else Err("Unknown transcription
  model: <name>"). Uses `enqueue_with_model`.
- Worker: reads `job_model(&id)` together with the cancel flag; model = job model if in MODELS, else Settings model, else DEFAULT.
- `edit_segment(meeting_id, revision, segment_id, text) -> Transcript`: refused while `meeting_busy` (being processed, recording or
  saving — a new Revision may land), then `Library::edit_segment`; emits `library-changed`. Register the command.

## Frontend

- `src/transcript.ts`:
  - `editSegment(meetingId, revision, segmentId, text)` → `invoke<Transcript>("edit_segment", { meetingId, revision, segmentId, text })`.
  - `currentSegmentId(segments, positionMs): string | null` — the Segment with `start_ms <= positionMs < end_ms`; when several
    overlap, the one starting latest; binary search on the sorted list (segments are sorted by start_ms).
  - `speakerName(speaker)`: Me / Remote / Unknown.
- `processing.ts`: `enqueueTranscription(meetingId, model?: string)` → invoke with `{ meetingId, model: model ?? null }`;
  `ProcessingJob.model: string | null`.
- `Player.tsx`: optional prop `onPosition?(ms: number)` called on timeupdate (and on seek).
- `src/TranscriptView.tsx` (props: transcript, positionMs, player: TimelinePlayer | null, editable: boolean,
  onEdit(segmentId, text): Promise<void>):
  - `<ol aria-label="Segments">`, one `<li>` per Segment: a button labelled "[hh:mm:ss] <Speaker>" (`formatElapsed` of start_ms)
    that calls `player.playSpan(start_ms, end_ms)` (disabled with no player), the text, and an "Edit" button (disabled when not
    editable).
  - the current Segment (`currentSegmentId(…, positionMs)`) gets `aria-current="true"` and class `current`; scroll it into view
    only if `scrollIntoView` exists (jsdom lacks it) and only when the current ID changes.
  - Edit: textarea (label "Segment text") prefilled; Save (disabled when trimmed empty or saving) / Cancel; Ctrl+Enter saves,
    Escape cancels. Errors from onEdit show with `role="alert"` in the row and keep the editor open.
- `MeetingView.tsx`:
  - position state from Player `onPosition`; layout: player and TranscriptView side by side (`.meeting-body` grid: player column +
    transcript column; stacks under 800px).
  - Header line: "Transcript revision N · model M · <count> segments · <language>".
  - Re-transcribe row when a transcript exists and no job: select "Model" (`""` = "Model from Settings", then base / small /
    medium / large-v3-turbo) + "Re-transcribe" → `enqueueTranscription(id, model || undefined)`. Language uses the existing picker.
    Short note: "Re-transcribing adds a new revision; earlier revisions are kept."
  - onEdit → `editSegment(id, transcript.revision, …)` then `setTranscript(result)`. Editing disabled while a job for this Meeting
    is not failed.
- `style.css`: `.meeting-body`, `.segments li.current` highlight, compact rows.

## Tests

- core `tests/library.rs`: edit persists to the current Revision's JSON (ID, range, speaker, revision unchanged) and to
  `transcript.md`; whitespace collapsed; empty → EmptySegmentText; unknown segment → SegmentNotFound; editing r1 after r2 exists →
  StaleRevision and r1 file unchanged; `transcript_revision` reads r1 after r2 exists and `segment("s00001")` resolves there
  (Citation against r1) while `transcript()` returns r2; missing revision → None.
- core `tests/processing.rs`: `enqueue_with_model` sets/updates the model; running job rerun picks the new model; `enqueue` → None.
- frontend: `transcript.test.ts` (currentSegmentId: before first, inside, at end boundary exclusive, gap, overlap → latest start);
  `TranscriptView.test.tsx` (click plays span, current highlight, edit save/cancel/error, disabled when not editable);
  MeetingView: re-transcribe with chosen model calls `enqueueTranscription("old", "medium")`, default calls with no model.
