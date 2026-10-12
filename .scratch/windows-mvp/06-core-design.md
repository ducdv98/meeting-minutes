# Ticket 06 design: crash recovery and multi-Recording Meetings (crates/core)

Ticket: .scratch/windows-mvp/issues/06-crash-recovery.md. Vocabulary: CONTEXT.md. Ticket 05 design: 05-core-design.md.
Existing code: crates/core/src/{recording,recorder,library,meeting_file}.rs. Keep names/signatures below exactly;
the Tauri app is written against them in parallel.

## Behaviour

- After a crash/kill, each `<meeting>/.chunks/recording-<N>/` left on disk is an orphan. At startup the app queues
  every orphan as a save (same path as Stop -> finalize_recording), so a failed recovery shows Retry save.
- Truncated Chunks are recovered, losing at most the Chunk in progress:
  - `<source>-<NNNNNN>.part`: truncate to whole frames (mono f32 = 4 bytes, stereo = 8), then rename to `.f32`.
    If the `.f32` of that number already exists, return an error (do not overwrite).
  - `<source>-<NNNNNN>.f32` whose size is not a whole number of frames: truncate the trailing partial frame (set_len).
  This salvage step runs at the start of every `finalize_recording` (idempotent; no-op for a clean Stop).
- Continue: a Meeting gets Recording N+1. Its Timeline starts at the sum of earlier Recording durations, preceded by
  a gap Marker. `meeting.mp3` is rebuilt to cover the whole Timeline.

## Timeline maths (new `pub mod timeline`, pure, unit tested)

```rust
// recording.rs: Marker gains a Gap variant.
pub enum Marker { Pause { at_ms: u64, paused_ms: u64 }, Gap { at_ms: u64 } }   // json: {"kind":"gap","at_ms":..}

// timeline.rs
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement { pub timeline_start_ms: u64, pub markers: Vec<Marker> }

/// Places a Recording after the Meeting's earlier Recordings (`earlier` = their duration_ms, in order).
/// timeline_start_ms = sum(earlier). If `earlier` is non-empty, markers start with Gap { at_ms: timeline_start_ms }.
/// The Recording's own Markers (relative to its start) are offset by timeline_start_ms; their at_ms is clamped to
/// `duration_ms` (a recovered Recording can lose the Chunk in progress, so a pause can lie past its end).
pub fn place_recording(earlier: &[u64], duration_ms: u64, markers: &[Marker]) -> Placement;
```

`finalize_recording` uses `place_recording` instead of its inline offset code.

## finalize_recording changes (recording.rs)

Order of operations for `finalize_recording(meeting_dir, number)`:
1. Read meeting.json. If it already lists Recording `number` (a kill after meeting.json was written but before the
   chunk dir was removed): remove the chunk dir (ignore failure) and return Ok with that entry's values.
2. Every Recording 1..number-1 must be listed in meeting.json, else `Err(LibraryError::Encode("Recording K is not
   saved yet"))`.
3. Read recording.json (as today), salvage Chunks (above).
4. Leftovers of a killed earlier attempt: the chunk dir holds a journal file `finalizing` with one created file name
   per line (names relative to the meeting dir). Before creating each final file, append its name to the journal and
   `sync_all`. At the start of an attempt, remove every listed file that exists, then truncate the journal. On an
   in-process failure, remove created files (as today) and the journal entries. The journal goes away with the chunk
   dir on success. Only files listed in the journal are ever deleted.
5. FLACs: as today (`create_new`).
6. MP3 covers the whole Timeline: earlier Recordings in order, decoded from their FLAC files listed in meeting.json
   (mic mono + system stereo mixed as today; move `claxon` to [dependencies]), each contributing exactly
   `duration_ms * 48` frames (pad with silence or cut), then the new Recording from its Chunks (all its frames).
   Still streaming / bounded memory.
   - meeting.audio is None: create `meeting.mp3` with `create_new` (as today; a foreign `meeting.mp3` makes the
     save fail and is kept — existing recorder test).
   - meeting.audio is Some(name): encode to `<name>.tmp` (`create_new`, journalled), and after the encode succeeds
     rename it over `<name>` (this is our own derived file, replaced atomically like meeting.json), then write
     meeting.json.
7. meeting.json: push the Recording entry (timeline_start_ms from Placement), extend markers with
   Placement.markers, duration_ms = timeline_start_ms + duration_ms, audio = Some(name or "meeting.mp3").

## Library additions (library.rs)

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OrphanRecording { pub meeting_id: String, pub number: u32 }

impl Library {
    /// Every `<meeting>/.chunks/recording-<N>/` dir in the Library (skip dot folders at the root, ignore names that
    /// don't parse), sorted by meeting_id then number.
    pub fn orphan_recordings(&self) -> Result<Vec<OrphanRecording>, LibraryError>;
    /// Starts Recording N+1 of an existing Meeting: N = highest Recording number in meeting.json (0 if none).
    /// Errors with LibraryError::UnsavedRecording if any `.chunks/recording-*` dir exists in that Meeting (its
    /// Recording must be saved first so the Timeline start is known). Creates the chunk dir with create_dir (not
    /// _all for the last level; fail if it exists) and writes recording.json with duration_ms None, no markers.
    pub fn begin_next_recording(&mut self, meeting_id: &str, started_at: DateTime<FixedOffset>)
        -> Result<(PathBuf, RecordingManifest), LibraryError>;
}
```
`LibraryError::UnsavedRecording` displays as "This Meeting has a Recording that is not saved yet". Export
`OrphanRecording` from lib.rs.

## Recorder additions (recorder.rs)

```rust
pub struct PendingSave { pub meeting_id: String, pub state: SaveState, pub error: Option<String>, pub recovered: bool }
pub struct RecorderStatus { ..existing.., pub continue_offers: Vec<String> }   // meeting ids, in recovery order
pub enum RecorderError { ..existing.., SavePending }   // "That Meeting is still being saved"

impl Recorder {
    /// Queues every orphan Recording in the Library as a recovered save (state Saving, recovered: true) and returns
    /// the jobs in orphan order; the caller runs them one by one (same meeting must be sequential) and reports each
    /// with save_finished. Orphans whose meeting already has a save in the list are skipped.
    pub fn recover(&mut self, library: &Library) -> Result<Vec<SaveJob>, RecorderError>;
    /// save_finished(Ok) of a recovered save adds its meeting_id to continue_offers (no duplicates).
    /// Continues a Meeting with a new Recording. Same checks as start (NoticeRequired, AlreadyActive); SavePending if
    /// a save (saving or failed) exists for that Meeting. Removes the meeting's continue offer on success.
    pub fn continue_meeting(&mut self, library: &mut Library, meeting_id: &str, started_at: DateTime<FixedOffset>)
        -> Result<(), RecorderError>;
    pub fn dismiss_continue(&mut self, meeting_id: &str);
}
```
The save list is keyed by meeting_id; `continue_meeting` refusing while a save is pending keeps one save per Meeting.

## Tests (TDD, behaviour through the public API; add to crates/core/tests/)

- timeline: first Recording has no gap and offsets nothing; second starts at sum with Gap first; pauses offset;
  three Recordings with pauses; pause past the end clamped.
- recovery: chunk dir with a `.part` holding 10.5 frames (stereo) and a `.f32` with a trailing partial frame ->
  finalize succeeds with the whole frames; a recovered Meeting is playable (FLAC decodes, mp3 present, meeting.json
  duration). Files listed in a `finalizing` journal from a "killed" attempt are removed and re-created; a foreign
  file not in the journal is still kept (existing test). Already-finalised Recording with a leftover chunk dir ->
  Ok, chunk dir removed, meeting.json unchanged.
- multi-Recording: finalize Recording 1, begin_next_recording -> number 2, write chunks, finalize -> meeting.json
  has 2 recordings (timeline_start_ms of 2 = duration of 1), markers include Gap at that point, duration is the sum,
  meeting.mp3 decodes to >= total frames (minimp3 includes delay; or parse the LAME tag like the existing gapless
  test: audio frames*1152 - delay - padding == total frames), and recording-1 FLACs untouched.
- finalize Recording 2 before Recording 1 is saved -> Err, nothing created.
- begin_next_recording with an unsaved chunk dir -> UnsavedRecording.
- orphan_recordings lists chunk dirs across meetings, sorted.
- Recorder: simulate a crash by starting a Recording, writing time, and dropping the Recorder without stop (use
  `std::mem::forget`-free approach: drop the Recorder; capture threads exit on Disconnected, leaving Chunks and maybe
  a .part); a new Recorder.recover lists it as recovered Saving; job.run() ok; save_finished -> continue_offers has
  it; continue_meeting starts Recording 2 of the same meeting; stop + run -> meeting.json has 2 recordings.
  continue_meeting while a save is pending -> SavePending.
