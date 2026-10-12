# Ticket 09 design: local transcription into Segments

Ticket: .scratch/windows-mvp/issues/09-local-transcription.md. Vocabulary: CONTEXT.md. Spec: ../spec.md (Processing). Research:
docs/research/windows-transcription-ask.md (section 5 is the Segment contract). Earlier designs: 05–08-core-design.md.
Existing code: crates/core/src/{library.rs, meeting_file.rs, recording.rs (FLAC/claxon reading in encode_mp3), recorder.rs, settings.rs},
src-tauri/src/main.rs, src/{App.tsx, MeetingView.tsx, library.ts, recording.ts}.

## Shape

- `crates/core` gets everything platform-independent and unit-tested with a fake engine: audio preparation, normalisation, IDs,
  speaker labels, Timeline offsets, transcript files, the model catalogue/downloader, and the Processing queue.
- New crate `crates/whisper` (package `meeting_minutes_whisper`, workspace member) holds the only `whisper-rs = "=0.16.0"` code: an
  implementation of core's `SpeechEngine` trait. src-tauri depends on it. Linux CI keeps running only `-p meeting_minutes_core`, so
  cmake is needed only on Windows CI (windows-latest has cmake + MSVC). No GPU features in v1 (CPU baseline).

## Files (Meeting folder)

`transcript.r<N>.json`, written and synced as `transcript.r<N>.json.tmp`, then published with a hard link (never overwritten). N = 1 + highest existing `transcript.r*.json` (so a re-transcribe
makes a new Revision). Current Revision = highest N.

```json
{
  "schema_version": 1,
  "revision": 1,
  "created_at": "2026-10-12T10:00:00+07:00",
  "model": "small",
  "language": "vi",                // language actually used (chosen or detected): "en" | "vi"
  "language_detected": true,       // false when the Meeting language was set by the user
  "duration_ms": 3600000,          // Meeting duration_ms at transcription time
  "segments": [
    { "id": "s00001", "start_ms": 0, "end_ms": 4200, "text": "Xin chào", "speaker": "me" }   // "me" | "remote" | "unknown"
  ]
}
```

`transcript.md` is derived from the current Revision and rewritten (temp file + rename) after each new Revision:

```
# <Meeting title>

Transcript revision 1 · model small · Vietnamese

**[00:00:00] Me:** Xin chào

**[00:00:04] Remote:** ...
```

## Core modules

### `transcript` (pub mod)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Speaker { Me, Remote, Unknown }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language { En, Vi }       // json "en" / "vi"; meeting.json `language` uses the same strings, null = auto-detect

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment { pub id: String, pub start_ms: u64, pub end_ms: u64, pub text: String, pub speaker: Speaker }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transcript { schema_version, revision: u32, created_at: String, model: String, language: Language,
                        language_detected: bool, duration_ms: u64, segments: Vec<Segment> }

/// Engine output for one audio buffer; times are ms from the start of that buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawSegment { pub start_ms: u64, pub end_ms: u64, pub text: String }

/// Per-Source activity for one Recording, one entry per 20 ms frame (`ACTIVITY_FRAME_MS`) of the Recording:
/// RMS of that frame. Imported/mono audio has `None` sources and labels everything Unknown.
pub struct Activity { pub microphone: Option<Vec<f32>>, pub system_audio: Option<Vec<f32>> }

pub const ACTIVITY_FRAME_MS: u64 = 20;
pub const ACTIVE_RMS: f32 = 0.01;          // ≈ -40 dBFS
pub const ACTIVE_FRACTION: f32 = 0.15;     // a Source is active in a span when ≥15% of its frames are ≥ ACTIVE_RMS
pub const ACTIVE_MIN_FRAMES: usize = 25;    // or ≥25 active frames (500 ms), even in a long span

/// Me: only the Microphone is active. Remote: only System audio. Unknown: both (overlap/bleed), neither, or no Source split.
pub fn speaker_label(activity: &Activity, start_ms: u64, end_ms: u64) -> Speaker;   // ms relative to the Recording

/// Normalises one Recording's engine output onto the Timeline:
/// trims text and collapses inner whitespace; drops empty text and pure non-speech tags (whole text matching `[...]` or `(...)`,
/// e.g. "[BLANK_AUDIO]", "(music)"); clamps each range to [0, recording_ms]; drops ranges with start >= end;
/// labels the speaker from `activity`; adds `timeline_start_ms`. Returns segments without IDs, sorted by (start_ms, end_ms).
pub fn place_segments(raw: &[RawSegment], recording_ms: u64, timeline_start_ms: u64, activity: &Activity) -> Vec<PlacedSegment>;
pub struct PlacedSegment { pub start_ms: u64, pub end_ms: u64, pub text: String, pub speaker: Speaker }

/// Joins all Recordings' placed segments, sorts by (start_ms, end_ms), clamps end_ms to `duration_ms` (dropping start >= end),
/// and assigns IDs "s00001", "s00002", … in that order (zero-padded to 5 digits, wider if ever needed).
pub fn assign_ids(placed: Vec<PlacedSegment>, duration_ms: u64) -> Vec<Segment>;

pub fn render_markdown(title: &str, transcript: &Transcript) -> String;   // format above; times via [hh:mm:ss] of start_ms
```

Reading: `Library::transcript(id) -> Result<Option<Transcript>, LibraryError>` returns the current Revision (highest N), or None.
IDs are stable across reloads because they are stored, never recomputed.

### `audio_prep` (module inside core)

```rust
pub const WHISPER_RATE: u32 = 16_000;
pub struct PreparedRecording { pub samples_16k: Vec<f32>, pub activity: Activity, pub duration_ms: u64, pub timeline_start_ms: u64 }
/// Streams `recording-<N> microphone.flac` (mono) and `recording-<N> system.flac` (stereo) with claxon — never holds 48 kHz
/// audio in memory: mixes mic + mean(system L,R) per frame, hard-clips to [-1,1], low-pass filters and decimates 48k → 16k
/// (windowed-sinc FIR, cutoff ~7.5 kHz, ≥ 31 taps, decimate by 3), and computes the 20 ms RMS of each Source separately.
pub fn prepare_recording(meeting_dir: &Path, recording: &RecordingFile) -> Result<PreparedRecording, ProcessingError>;
```

Memory for 60 min: 16 kHz mono f32 ≈ 230 MB, plus ~180 k activity frames per Source. Acceptable.

### `processing` (pub mod)

```rust
pub trait SpeechEngine {
    /// Picks English or Vietnamese from the first 30 s (whichever of the two the model rates higher).
    fn detect_language(&mut self, samples_16k: &[f32]) -> Result<Language, String>;
    /// `progress` gets 0.0..=1.0 for this buffer. Returns Err(Cancelled) promptly once `cancel` is set.
    fn transcribe(&mut self, samples_16k: &[f32], language: Language, initial_prompt: Option<&str>,
                  progress: &mut dyn FnMut(f32), cancel: &AtomicBool) -> Result<Vec<RawSegment>, ProcessingError>;
}

pub enum ProcessingError { Cancelled, Io(io::Error), Json(serde_json::Error), Audio(String), Engine(String),
                           Download(String), Checksum, NoAudio }   // Display + Serialize-as-string like LibraryError

pub struct TranscribeRequest<'a> { pub meeting_dir: &'a Path, pub model: &'a str, pub vocabulary: &'a [String] }

/// Reads meeting.json; for each Known Recording in order: prepare_recording, then transcribe; language = meeting.json language,
/// or detect_language on the first Recording when null (auto). Progress across Recordings is weighted by duration.
/// Writes transcript.r<N>.json.tmp (sync), publishes transcript.r<N>.json with hard_link, then transcript.md (tmp+rename).
/// A Meeting with no Recordings → NoAudio (Import is ticket 11).
pub fn transcribe_meeting(request: &TranscribeRequest, engine: &mut dyn SpeechEngine,
                          progress: &mut dyn FnMut(f32), cancel: &AtomicBool) -> Result<Transcript, ProcessingError>;
```

Initial prompt: vocabulary terms (trimmed, non-empty) joined with ", "; None when empty.

### `models` (pub mod) — model manager

```rust
pub struct ModelInfo { pub name: &'static str, pub file: &'static str, pub size: u64, pub sha256: &'static str }
pub const MODELS: &[ModelInfo] = &[ /* from huggingface.co/api/models/ggerganov/whisper.cpp/tree/main, checked 2026-10-12 */
  base           ggml-base.bin            147_951_465  60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe
  small          ggml-small.bin           487_601_967  1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b
  medium         ggml-medium.bin        1_533_763_059  6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208
  large-v3-turbo ggml-large-v3-turbo.bin 1_624_555_275 1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69
];
pub const DEFAULT_MODEL: &str = "small";
pub fn model_url(info: &ModelInfo) -> String;  // https://huggingface.co/ggerganov/whisper.cpp/resolve/main/<file>
pub fn model_path(models_dir: &Path, info: &ModelInfo) -> PathBuf;
/// A model is installed when its file exists with the expected size (the hash is checked once, at download).
pub fn is_installed(models_dir: &Path, info: &ModelInfo) -> bool;
/// Copies `body` to <file>.part while hashing (sha2), reporting bytes done; checks size + sha256; renames to the final file.
/// On cancel, error or mismatch, removes the .part. Never overwrites an existing final file (it is just reused if installed).
pub fn install_from_reader(models_dir: &Path, info: &ModelInfo, body: impl Read, progress: &mut dyn FnMut(u64),
                           cancel: &AtomicBool) -> Result<PathBuf, ProcessingError>;
/// HTTP GET (ureq 2.x with rustls; follows redirects) feeding install_from_reader. Not unit tested.
pub fn download(models_dir: &Path, info: &ModelInfo, progress: &mut dyn FnMut(u64), cancel: &AtomicBool) -> Result<PathBuf, ProcessingError>;
```

Only multilingual checkpoints are listed (no `.en`, no tiny). Unknown model name in settings falls back to `small`.

### Processing queue (`processing::Processor`), like `Recorder`

```rust
#[derive(Serialize)] #[serde(rename_all = "snake_case")]
pub enum JobStage { Queued, Downloading, Transcribing, Failed }
#[derive(Serialize)] pub struct ProcessingJob { pub meeting_id: String, pub stage: JobStage, pub progress: f32, pub error: Option<String> }
#[derive(Serialize)] pub struct ProcessingStatus { pub jobs: Vec<ProcessingJob> }

impl Processor {
    pub fn new() -> Self;
    pub fn enqueue(&mut self, meeting_id: &str) -> bool;      // running job gets one rerun; a Failed job is re-queued
    pub fn next(&mut self) -> Option<String>;                  // first Queued → becomes the running job (stage set by worker)
    pub fn set_stage(&mut self, meeting_id: &str, stage: JobStage, progress: f32);
    pub fn finished(&mut self, meeting_id: &str, result: Result<(), ProcessingError>);  // Ok → removed or rerun; Cancelled → removed; Err → Failed
    pub fn cancel(&mut self, meeting_id: &str);                // queued → removed; running → sets the shared cancel flag
    pub fn cancel_flag(&self) -> Arc<AtomicBool>;              // the running job's flag (reset when a job starts)
    pub fn is_busy(&self, meeting_id: &str) -> bool;           // queued or running (used to refuse rename)
    pub fn is_running(&self) -> bool;                          // Quit waits until the running job finishes
    pub fn dismiss(&mut self, meeting_id: &str);               // drops a Failed job
    pub fn status(&self) -> ProcessingStatus;
}
```

One job runs at a time. Unit-test the queue transitions and `transcribe_meeting` with a fake engine.

## crates/whisper

`WhisperEngine::load(model_path) -> Result<Self, String>` (WhisperContext, `use_gpu(false)`), impl `SpeechEngine`:
- `detect_language`: new state, `pcm_to_mel` on the first ≤30 s, `lang_detect(0, threads)` → probs; compare `probs[lang_id("en")]` vs
  `probs[lang_id("vi")]` (use `whisper_rs::get_lang_id`).
- `transcribe`: `FullParams::new(SamplingStrategy::Greedy { best_of: 1 })`, `set_language(Some("en"|"vi"))`, `set_translate(false)`,
  `set_print_*(false)`, `set_suppress_blank(true)`, `set_suppress_nst(true)`, `set_initial_prompt` when Some,
  `set_n_threads(available_parallelism - 1, clamped 1..=8)`, progress via `set_progress_callback_safe` (percent / 100),
  abort via `set_abort_callback_safe` reading the cancel flag; after `full`, if cancel is set → `Err(Cancelled)`.
  Segments: t0/t1 are in 10 ms units → ms. Read the segment text lossily (invalid UTF-8 from split multibyte tokens must not fail).
- One `#[ignore]` integration test that downloads nothing: runs only when `MM_WHISPER_MODEL` points to a ggml model file and
  transcribes 2 s of silence + checks it returns without error (sanity of the binding).

## Settings (settings.rs)

Add `#[serde(default)]` on the struct (old settings files must keep loading — today a parse error resets the notice flag!) and:
`auto_process: bool` (default **true**, so hand-write `Default`), `model: String` (default "small"), `vocabulary: Vec<String>`.

## Library

- `Library::set_language(id, language: Option<Language>)`: updates only `"language"` in meeting.json (Value-based tmp+rename like
  `rename_meeting`; share that helper). Same id checks (`checked_meeting_dir`).
- `Library::transcript(id)` as above. `MeetingDetail` stays as is; the UI calls `meeting_transcript` separately.

## Tauri (main.rs)

- State: `Mutex<Processor>`, plus `models_dir = app_data_dir/models`. A single worker thread started in setup loops: waits (Condvar or
  100 ms sleep poll, matching existing style) for `next()`; does not start a job while the recorder phase is not Idle (capture first);
  loads settings; resolves model; if not installed → stage Downloading with progress = bytes/size, `models::download`; then stage
  Transcribing, `WhisperEngine::load`, `transcribe_meeting` (progress updates throttled to ≥1% steps); `finished`.
  Emits `processing-status` on every change and `library-changed` after success.
- `spawn_saves`: after a successful save, `if Settings::load(..).auto_process { processor.enqueue(meeting_id) }`.
- Commands: `processing_status`, `transcribe_meeting(meeting_id)` (enqueue), `cancel_processing(meeting_id)`,
  `dismiss_processing(meeting_id)`, `meeting_transcript(meeting_id) -> Option<Transcript>`, `set_meeting_language(meeting_id,
  language: Option<Language>)`, `get_settings`, `update_settings(auto_process, model, vocabulary)` (keeps
  `recording_notice_acknowledged`).
- `rename_meeting` also refuses while `processor.is_busy(id)` (same message style: "This Meeting is being processed").
- `set_meeting_language` refused while busy too.
- `quit`: cancel the running job before waiting for saves.

## Frontend

- `src/processing.ts`: types + invoke wrappers + `onProcessingStatus`.
- MeetingView "Transcript" section:
  - Language select (label "Language"): Auto-detect / English / Vietnamese → `setMeetingLanguage`; replaces the "Language:" line.
  - No transcript and no job → "Transcribe" button. Job Queued → "Waiting to transcribe" + Cancel. Downloading → "Downloading model
    <n>%" (`<progress>`) + Cancel. Transcribing → "Transcribing <n>%" (`<progress>`) + Cancel. Failed → error (`role="alert"`) +
    Retry + Dismiss.
  - Transcript present → "Transcript revision N · <count> segments · <language>" and a "Re-transcribe" button (new Revision).
    The full transcript view is ticket 10; do not build it here.
  - Reload the transcript when `library-changed` fires for this view.
- Settings view (header button "Settings", same no-router approach as the Meeting view): checkbox "Transcribe automatically after
  recording", select "Transcription model" (base / small (default) / medium / large-v3-turbo with sizes in MB), textarea
  "Vocabulary" (one term per line), Save.

## Tests

- core unit tests (tests/transcript.rs, tests/processing.rs, tests/models.rs):
  normalisation (trim/collapse, drop empty + `[BLANK_AUDIO]`, clamp, drop zero-length, sort); ID assignment and stability
  (write → `Library::transcript` → same IDs); Timeline offsets across two Recordings (second starts at its `timeline_start_ms`, clamped to
  Meeting duration); speaker labels (mic-only → Me, system-only → Remote, both → Unknown, neither → Unknown, no split → Unknown,
  threshold edge at 15%); `transcribe_meeting` with a fake engine over real FLACs written via existing recording helpers
  (or `finalize_recording` of synthetic Chunks): writes r1 then r2 on a second run, transcript.md content, auto-detect path, vocabulary
  prompt passed, cancel writes nothing, NoAudio; 48k→16k prep: length = ceil(frames/3), a 1 kHz tone survives, a 10 kHz tone is
  attenuated ≥ 20 dB; models: checksum mismatch removes .part and errors, cancel removes .part, success renames; Processor transitions;
  Settings: old file `{"recording_notice_acknowledged":true}` loads with the new defaults.
- Frontend: MeetingView transcript states (Transcribe → enqueue, progress + Cancel, failed + Retry, ready summary, language select);
  Settings view load/save.
