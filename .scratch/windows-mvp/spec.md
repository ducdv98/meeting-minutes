# Windows MVP spec

Status: ready-for-agent

Personal, local-first Windows 11 x64 meeting recorder. Vocabulary: `CONTEXT.md`. Decisions: `docs/adr/0001`–`0003`. Feasibility evidence: `docs/research/`.

## Goal

One person records any call (Zoom, Teams, Meet, …), and the app saves it to their Library, transcribes it on-device, summarises it with cited items, and answers cited questions about that one Meeting, with playback of the cited audio.

## User loop

Record (Microphone + System audio) → Stop → Meeting saved to Library → local Processing (Transcript → Summary + Action items) → read and correct the Transcript → Ask with Citations → click a Citation to play that span of the Timeline.

## Requirements

### Capture
- Recording starts only on deliberate user action: window button, tray menu, or global hotkey. Never automatic.
- First Recording ever shows the Recording notice once; a recording indicator is always visible while a Recording runs (window + tray).
- Captures Microphone and System audio (WASAPI endpoint loopback of the default render device) as separate Sources. No per-process loopback in v1 (ADR-0003).
- Pause/resume keeps the same Recording and adds a pause Marker.
- Audio is written as Chunks; a crash, kill or power loss loses at most the Chunk in progress.
- On default-device change (either direction), capture follows the new default device, keeps the same Recording, adds a device-switch Marker, and shows a non-blocking notice.
- Only one Recording can be active at a time.

### Library
- Library root: `Documents\Meeting Minutes\`. One folder per Meeting: `<YYYY-MM-DD HHmm> <title>\`.
- Meeting folder holds: one FLAC per Source per Recording, a mixed MP3 of the whole Timeline, `meeting.json` (metadata, Recordings, Markers, language), `transcript.r<N>.json` per Revision, `transcript.md`, `summary.json`, `summary.md`, `asks.json`.
- An SQLite index in app data powers lists and Library search; it can be fully rebuilt from the Library folders.
- Never overwrite an existing folder or file; name collisions get a distinct suffix.
- Deleting a Meeting moves its folder to the Recycle Bin and removes it from the index.
- Orphan Chunks found at startup are recovered into a Recording of their Meeting (crash recovery). Starting a new Recording within a Meeting after a crash adds a Recording to that same Meeting.
- Import of an audio file (mp3, m4a, wav, flac) creates a Meeting with no Source split.

### Processing (local by default, ADR-0002)
- After Stop, local Processing starts automatically unless the user turned off the setting; cloud Processing only ever runs on an explicit click.
- Transcription: whisper.cpp via `whisper-rs`, multilingual models only. Default `small`; `base`/`medium`/`large-v3-turbo` selectable. Models download on first use with progress, checksum and cancel.
- Meeting language: English or Vietnamese, chosen per Meeting or auto-detected.
- Vocabulary from settings is passed as the initial prompt.
- Segments get app-assigned stable IDs and integer ms ranges on the Timeline (`0 <= start_ms < end_ms <= duration_ms`).
- Speaker label per Segment: Me (Microphone only), Remote (System audio only), Unknown (both active / bleed). Imported Meetings use Unknown.
- Re-transcribing creates a new Revision; editing a Segment's text keeps its ID and Revision.
- Summary in fixed sections: Overview, Key points, Decisions, Action items (owner/due date when stated). Every item needs ≥1 valid Citation; items without one are dropped.

### Ask (Meetless rules)
- Scoped to one Meeting's current Transcript Revision; nothing from other Meetings is sent.
- Answers cite Segment IDs; app checks each ID exists in that Meeting and Revision. Visible times are derived from stored ms ranges only.
- Explicit Insufficient evidence result when the Transcript has no support.
- History persists per Meeting across restarts. A failed question stays visible and can be retried; retry only for the latest failed question.
- At most one active Ask per Meeting; Stop keeps partial text labelled Incomplete; invalid Citations → Unverified. Incomplete/Unverified Answers are excluded from later context.
- Off-topic input gets a polite nudge back to the Meeting.

### AI providers
- Local: Ollama if installed (detect at `localhost:11434`, list models, let user choose).
- Cloud: user's own OpenAI or Anthropic API key, stored in Windows Credential Manager. A cloud send shows a disclosure of what will be sent, with consent recorded once per Meeting; every send is still an explicit click.
- If neither is available, Summary/Ask show setup guidance; recording and transcription still work.

### Library search
- Keyword search over all current Transcripts; results jump to the Segment and can play it.

## Out of scope (v1)
Cross-meeting Ask, calendar, bots joining calls, live transcription, identifying individual remote speakers, echo cancellation, meeting auto-detection, editable summary templates, bundled llama.cpp, Arm64, code signing, billing, Microsoft Store. (The public Free Beta is planned in `.scratch/public-beta/`; code signing comes before leaving Beta, ADR-0007.)

## Testing
- Rust core: unit tests for Timeline maths, Chunk recovery, Library index rebuild, citation validation, Segment normalisation.
- Windows-only capture behaviour is verified manually on the user's laptop (see human tickets).
- CI: GitHub Actions `windows-latest` builds the installer and runs tests on every push.
