# Ticket 11 design: Import an audio file as a Meeting

Ticket: .scratch/windows-mvp/issues/11-import-audio.md. Vocabulary: CONTEXT.md (Import: "An imported Meeting has no Source split";
Relationships: "an imported Meeting has no Recordings and holds its audio directly"). Earlier designs: 05–10-core-design.md.
Existing code: crates/core/src/{library.rs, meeting_file.rs, audio_prep.rs, processing.rs}, src-tauri/src/main.rs, src/App.tsx.

Dependencies are already added: core `symphonia = 0.6.1` (features mp3, aac, isomp4, wav, pcm, flac; no defaults), src-tauri
`tauri-plugin-dialog = "=2.8.2"`. Do not add others.

## Meeting folder of an import

- The original is **copied** (never moved, never opened for writing) to `imported.<ext>` (`ext` = the source extension lowercased:
  mp3 | m4a | wav | flac). That copy is the playback file: `meeting.json` `audio = "imported.<ext>"` (WebView2 plays all four).
  No MP3 is made, no Recordings, no Markers.
- `meeting.json` gains an optional field (serde default None, skipped when None):
  ```json
  "import": { "file": "imported.m4a", "original_name": "Team sync.m4a", "imported_at": "2026-10-12T10:00:00+07:00" }
  ```
  `duration_ms` = decoded duration of the audio (ms, rounded down). `title` = original file stem (trimmed; "Imported audio" when
  empty). `started_at` = import time (local, with offset). Language null (auto-detect).
- Crash safety / never overwrite: build the Meeting in a staging folder `<root>/.import-<pid>-<nanos>` (dot folders are already
  skipped by rescan and orphan recovery): copy the audio to `imported.<ext>.tmp`, sync, rename to `imported.<ext>`, write
  `meeting.json` (existing `write_meeting_file`), then rename the staging folder to the final name using the same
  `folder_name` + ` (n)` collision loop as `create_meeting` (only to a name that does not exist). On any error remove the staging
  folder. Then insert the index row.

## Core

### `import` (pub mod, new file crates/core/src/import.rs)

```rust
pub const IMPORT_EXTENSIONS: &[&str] = &["mp3", "m4a", "wav", "flac"];

/// Streams the file with symphonia (probe by extension hint), calling `on_samples(mono_f32, sample_rate)` for each decoded packet
/// (channels averaged to mono). Skips recoverable decode errors (`symphonia::core::errors::Error::DecodeError`) and stops at end of
/// stream. Errors: unsupported extension / no default audio track / unsupported codec (e.g. HE-AAC) / a sample rate outside
/// `IMPORT_RATES` (8–192 kHz, checked on the first packet so import refuses what Processing could not transcribe) → Unsupported(…);
/// I/O → Io. Checks `cancel` between packets → Cancelled. Sample-rate changes mid-stream → Unsupported.
pub fn decode_audio(path: &Path, cancel: &AtomicBool, on_samples: &mut dyn FnMut(&[f32], u32)) -> Result<(), ProcessingError>;

/// Decoded duration in ms (frames / rate * 1000, rounded down) — decodes the whole file, which also validates it.
pub fn audio_duration_ms(path: &Path, cancel: &AtomicBool) -> Result<u64, ProcessingError>;
```

Reuse `ProcessingError` (add `Unsupported(String)` with Display "Unsupported audio file: …"). No new error type.

### library.rs

```rust
/// Validates the extension (case-insensitive, IMPORT_EXTENSIONS) and decodes the file for its duration (0 ms → error
/// "This audio file has no sound to import" as LibraryError::Encode); then builds the Meeting as above. `source` is only read.
pub fn import_audio(&mut self, source: &Path, imported_at: DateTime<FixedOffset>, cancel: &AtomicBool)
    -> Result<MeetingSummary, LibraryError>;
```

Map ProcessingError → `LibraryError::Encode(error.to_string())` (Cancelled → `LibraryError::Encode("Import cancelled")`).

`begin_next_recording` must refuse an imported Meeting (`meeting.import.is_some()`) with a new `LibraryError::ImportedMeeting`
("An imported Meeting cannot get new Recordings"). `MeetingDetail` gains `imported: bool`.

### audio_prep.rs — any sample rate

```rust
/// Decodes an imported file (via import::decode_audio) to 16 kHz mono for whisper. Streaming: never holds the source-rate audio;
/// an exact polyphase windowed-sinc resampler (cutoff 0.45 × min(in_rate, 16k) relative to in_rate, Blackman window, 16 zero
/// crossings per side at the lower rate, weights precomputed per phase) converts any input rate (8k–192k) to 16k.
/// activity = Activity { microphone: None, system_audio: None } (→ Speaker label Unknown), timeline_start_ms 0.
pub fn prepare_import(meeting_dir: &Path, file: &str, cancel: &AtomicBool) -> Result<PreparedRecording, ProcessingError>;
```

### processing.rs

`transcribe_meeting`: when `metadata.import` is Some → one PreparedRecording from `prepare_import(meeting_dir, &import.file)`,
duration weight 1.0, then the same detect/transcribe/place/assign/write path as Recordings. `NoAudio` only when neither
Recordings nor import exist. Factor the per-buffer step so both paths share it (no duplicated transcribe/place code).

## Tauri (main.rs)

- Register `tauri_plugin_dialog::init()`. No JS dialog package or capability is needed: the picker runs Rust-side.
- Import queue state `Mutex<ImportState { queue: VecDeque<PathBuf>, current: Option<String /* file name */>, errors: Vec<String> }>`
  and one import thread started on demand (or a loop like the processing worker). Each import: `Library::import_audio` holding
  `ImportStatus` is defined in main.rs beside the queue. Each import holds the library lock **only** for the final folder
  rename + index insert; the staged copy (not the source) is what gets decoded and measured — decoding for the duration must not hold the library lock
  (split `import_audio` into `prepare_import_meeting` (no &mut self needed: staging + copy + decode) and `publish` if that is
  cleaner; keep the public behaviour above). After success: if `Settings.auto_process` → enqueue Processing, then emit `library-changed` (so a rename is refused from the
  moment the Meeting is visible until it is processed).
  Emit `import-status` `{ current: Option<String>, queued: usize, errors: Vec<String> }` on every change.
- Commands: `pick_audio_files()` → `app.dialog().file().add_filter("Audio", &["mp3","m4a","wav","flac"]).blocking_pick_files()`
  (run it off the main thread: `#[tauri::command(async)]`), queues the chosen paths; `import_status()`; `dismiss_import_errors()`.
- Drag-and-drop: in `on_window_event`, `WindowEvent::DragDrop(DragDropEvent::Drop { paths, .. })` → queue paths with a supported
  extension; unsupported ones add an error "<name>: not an mp3, m4a, wav or flac file". Also emit `import-hover` true/false on
  `Enter`/`Leave`/`Drop` for a drop hint.
- Quit waits for a running import like it waits for saves (cancel it via its AtomicBool; staging folder is removed).

## Frontend

- `src/importing.ts`: `ImportStatus` type, `pickAudioFiles()`, `importStatus()`, `dismissImportErrors()`, `onImportStatus`,
  `onImportHover`.
- Library header: "Import audio" button → `pickAudioFiles()`. While importing: "Importing <name>… (+N queued)" (`role="status"`).
  Errors list with `role="alert"` + Dismiss. While a file is dragged over the window: overlay "Drop audio to import (mp3, m4a, wav,
  flac)".
- Meeting view: for an imported Meeting (`detail.imported`) show "Imported audio" under the date; everything else (player,
  Transcribe, Transcript view) works unchanged.

## Tests

- core `tests/import.rs`: write small fixture files at test time — WAV (hand-written 16-bit PCM header, 44.1 kHz stereo, 1 kHz tone,
  2.5 s) and FLAC (existing flacenc helpers, 48 kHz mono); MP3 via mp3lame-encoder (already a dependency) at 44.1 kHz.
  Then: `import_audio` creates a Meeting folder with `imported.wav` byte-identical to the source, source unchanged (bytes + mtime),
  meeting.json fields (audio, import, duration within ±50 ms of truth, no recordings), index lists it; name collision gets ` (2)`;
  unsupported extension and corrupt file → error and no folder (no staging folder left either); cancel → no folder;
  `begin_next_recording` on an import → ImportedMeeting; rename of an imported Meeting keeps the audio playable (audio_path set).
- `prepare_import`: 44.1 kHz input length ≈ duration × 16 k (±1%); a 1 kHz tone survives (RMS within 1 dB), a 10 kHz tone at
  44.1 kHz attenuated ≥ 20 dB; activity both None.
- `transcribe_meeting` with the fake engine on an imported Meeting: r1 written, all Segments Unknown, ranges within duration.
- Frontend: Library import button calls `pickAudioFiles`; status/error rendering + Dismiss; drop overlay on hover event; Meeting
  view "Imported audio" line.
