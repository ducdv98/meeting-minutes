# Ticket 05 design: core recording API (crates/core)

Ticket: .scratch/windows-mvp/issues/05-record-a-meeting.md (read it, plus the ticket 03 comments "For ticket 05").
Vocabulary: CONTEXT.md. Existing code: crates/core/src/{library,meeting_file,index,naming}.rs.
Placement maths to reuse: spikes/capture/src/timeline.rs (copy/adapt into core; do not depend on the spike crate).

Everything below is platform-independent and unit-tested on Linux. The Windows WASAPI input and the
Tauri app are written separately against the traits below; keep the names/signatures exactly.

## Policies (decided)

- Drift: each Source is placed on the shared QPC clock by frame drop/pad (as in the spike). No resampling.
- Gain/clipping: samples are hard-clipped to [-1.0, 1.0] when converting to integers for encoding.
- Microphone is stored mono (average of the two captured channels). System audio stays stereo.
- FLAC: 16-bit, 48 kHz. Mixed MP3: stereo 48 kHz, 128 kbps CBR via `mp3lame-encoder`; mix = mic (mono, to both
  channels) + system, then hard-clipped.
- Chunks: raw little-endian f32 interleaved samples, 10 s per Chunk. In progress: `<source>-<NNNNNN>.part`;
  completed: flushed, `sync_all`, renamed to `<source>-<NNNNNN>.f32`. `<source>` = `microphone` | `system`.
- Chunk dir for Recording N of a Meeting: `<meeting folder>/.chunks/recording-<N>/`. It also holds
  `recording.json` (manifest, see below), rewritten via temp file + rename on every pause/resume/stop.
- Final files in the Meeting folder: `recording-<N> microphone.flac`, `recording-<N> system.flac`, `meeting.mp3`.
  They are created with `create_new` (never overwrite). If finalisation fails, every file created in that attempt is
  removed and the chunk dir is left untouched so Retry save works. After meeting.json + index are updated, the chunk
  dir for that Recording is removed (failure to remove is not an error).
- Finalising must stream (bounded memory): never hold a whole Source in memory. With flacenc use
  `encode_fixed_size_frame` per block and write STREAMINFO up front (total samples is known from chunk sizes;
  MD5 may be zero), or an equivalent streaming approach.

## Modules / API

### `capture` (pub mod)
```rust
pub const SAMPLE_RATE: u32 = 48_000;
pub const CAPTURE_CHANNELS: usize = 2;          // inputs always deliver 48 kHz stereo f32
pub const HNS_PER_SECOND: u64 = 10_000_000;     // clock unit is 100 ns (QPC converted)

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source { Microphone, SystemAudio }      // file stem: "microphone" / "system"; stored channels: 1 / 2

pub struct Packet { pub samples: Vec<f32> /* interleaved stereo */, pub qpc_100ns: Option<u64>, pub silent: bool, pub discontinuity: bool }

pub trait Clock: Clone + Send + 'static { fn now_100ns(&self) -> u64; }

/// Opened inside the Source's own thread (WASAPI objects are not Send).
pub trait InputDevice { fn read_packet(&mut self) -> Result<Option<Packet>, String>; }
pub trait InputOpener: Clone + Send + 'static {
    type Device: InputDevice;
    fn open(&self, source: Source) -> Result<Self::Device, String>;
}

/// Spike Timeline adapted: places packets on the Source timeline; supports pause/resume.
pub struct SourceClock { .. }
impl SourceClock {
    pub fn new(start_100ns: u64) -> Self;
    pub fn frames_written(&self) -> u64;
    pub fn packet(&mut self, packet: &Packet, arrival_100ns: u64) -> Vec<f32>; // placed samples (gap zeros + data)
    pub fn pad_to(&mut self, now_100ns: u64, final_pad: bool) -> Vec<f32>;   // idle pad (2 s margin unless final)
    pub fn pause(&mut self, at_100ns: u64) -> Vec<f32>;  // final pad to `at`, then ignore packets/pads
    pub fn resume(&mut self, at_100ns: u64);              // timeline continues at frames_written; packets with qpc < at are dropped
}
```

### `recording` (pub mod)
```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Marker { Pause { at_ms: u64, paused_ms: u64 } }   // at_ms on the Meeting Timeline

/// Control-level clock: Timeline ms excluding paused time, and pause Markers.
pub struct RecordingClock { .. }
impl RecordingClock {
    pub fn new(start_100ns: u64) -> Self;
    pub fn elapsed_ms(&self, now_100ns: u64) -> u64;   // frozen while paused
    pub fn is_paused(&self) -> bool;
    pub fn pause(&mut self, now_100ns: u64);            // no-op if already paused
    pub fn resume(&mut self, now_100ns: u64);           // pushes Marker::Pause { at_ms, paused_ms }
    pub fn markers(&self) -> &[Marker];
}

pub struct ChunkWriter { .. }
impl ChunkWriter {
    pub fn new(dir: &Path, source: Source) -> io::Result<Self>;           // 10 s chunks
    pub fn with_chunk_frames(dir: &Path, source: Source, frames: u64) -> io::Result<Self>; // for tests
    pub fn write(&mut self, stereo: &[f32]) -> io::Result<()>;            // downmixes Microphone to mono
    pub fn finish(self) -> io::Result<()>;                                // completes the in-progress chunk
}

#[derive(Serialize, Deserialize, ...)]
pub struct RecordingManifest { pub number: u32, pub started_at: String /*rfc3339*/, pub duration_ms: Option<u64>, pub markers: Vec<Marker> }

/// Two Source threads + control. Generic so tests use a fake clock and fake inputs.
pub struct CaptureSession<C: Clock> { .. }
impl<C: Clock> CaptureSession<C> {
    pub fn start<O: InputOpener>(chunk_dir: PathBuf, manifest: RecordingManifest, opener: O, clock: C) -> io::Result<Self>;
        // writes recording.json, spawns one thread per Source: loop { read control; open device if none (retry, log via
        // returned errors only); read packets -> SourceClock -> ChunkWriter; pad_to(now,false); sleep 10 ms }.
        // A Source whose device can't be opened is padded with silence (so the Timeline still advances).
    pub fn elapsed_ms(&self) -> u64;
    pub fn is_paused(&self) -> bool;
    pub fn pause(&mut self) -> io::Result<()>;   // both Sources pad to the same instant; manifest rewritten
    pub fn resume(&mut self) -> io::Result<()>;
    pub fn stop(self) -> io::Result<RecordingManifest>; // final pad to stop instant, finish chunks, join threads,
                                                       // manifest gets duration_ms and is rewritten
}
```

### Library additions (library.rs / meeting_file.rs)
- `meeting.json` stays schema_version 1 but becomes typed; old files with `recordings: []`/`markers: []` and no
  `duration_ms` must still parse (serde defaults):
```json
{ "schema_version": 1, "title": "...", "started_at": "...", "language": null,
  "duration_ms": 0, "audio": "meeting.mp3" | null,
  "recordings": [ { "number": 1, "started_at": "...", "timeline_start_ms": 0, "duration_ms": 1234,
                    "files": [ {"source": "microphone", "file": "recording-1 microphone.flac", "channels": 1},
                               {"source": "system_audio", "file": "recording-1 system.flac", "channels": 2} ] } ],
  "markers": [ { "kind": "pause", "at_ms": 500, "paused_ms": 2000 } ] }
```
- `Library::meeting_dir(&self, id: &str) -> PathBuf`
- `Library::begin_recording(&mut self, started_at: DateTime<FixedOffset>) -> Result<(MeetingSummary, PathBuf /*chunk dir*/, RecordingManifest), LibraryError>`
  creates a new Meeting titled "Untitled meeting" (folder via existing create_meeting) and its chunk dir for recording 1.
- `pub fn finalize_recording(meeting_dir: &Path, number: u32) -> Result<FinalizedRecording, LibraryError>` (free fn,
  no Library borrow, so the app can run it off the UI lock): reads `recording.json` + completed chunks, encodes FLACs
  + `meeting.mp3`, rewrites meeting.json (temp file + rename) appending the Recording entry (timeline_start_ms = sum of
  earlier Recordings' durations), Markers (offset by timeline_start_ms), `duration_ms`, `audio`. Removes the chunk dir.
  Duration = longest Source frame count (pad the shorter Source with silence in the files so both FLACs have the
  same length).
- `Library::record_finalized(&mut self, meeting_id: &str) -> Result<MeetingSummary, LibraryError>` refreshes the index
  row from meeting.json. Add `duration_ms: u64` to `MeetingSummary` and the index (bump index user_version → rebuild).
- `LibraryError` gains variants as needed (e.g. `Encode(String)`).

### `settings` (pub mod)
`Settings { recording_notice_acknowledged: bool }` with `Settings::load(path) -> Settings` (missing/invalid → default)
and `save(&self, path) -> io::Result<()>` (temp + rename).

## Tests (TDD, behaviour through the public API)
- SourceClock: pause pads to pause instant; resume continues; packets before resume dropped; spike tests kept.
- RecordingClock: elapsed excludes pause; marker at_ms/paused_ms; double pause no-op.
- ChunkWriter: rotation at N frames, `.part` while in progress, `.f32` after, mic mono downmix.
- CaptureSession with a fake clock (manually advanced, Arc<AtomicU64>) and fake inputs: stop yields chunk frames ==
  elapsed frames for both Sources; pause excludes paused time and adds Marker.
- finalize_recording: from fixture chunks → FLAC decodes (dev-dep `claxon`) with right length/channels; MP3 decodes
  (dev-dep e.g. `minimp3` or `symphonia`, pick what builds) with ≈ right duration; meeting.json updated; chunk dir removed;
  index updated after rescan/record_finalized.
- Failed finalisation (e.g. `recording-1 system.flac` already exists as a foreign file): returns Err, foreign file
  untouched, no new files left behind, chunks still there; after removing the blocker, retry succeeds.
- Old meeting.json fixtures (empty recordings, no duration) still parse.
- Settings load/save round-trip and default.
