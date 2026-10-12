# Record a Meeting end to end

Status: ready-for-agent
Blocked by: 03, 04
Spec: ../spec.md

## What

Turn the spike into the real capture module: Record / Pause / Resume / Stop from the window, tray and a global hotkey. Each Source is written as Chunks; Stop finalises one FLAC per Source per Recording plus a mixed MP3 of the Timeline into the Meeting folder, then updates `meeting.json` and the index. Show the Recording notice on the first Recording ever and a visible indicator while recording. Incorporate drift findings from ticket 03.

## Acceptance criteria

- [ ] Recording only starts on deliberate action; only one Recording can be active.
- [ ] Pause/resume stays in one Recording and adds a pause Marker; the Timeline excludes paused time.
- [ ] After Stop: FLAC per Source, mixed MP3, and `meeting.json` with duration, Recordings and Markers.
- [ ] Failed finalisation keeps the Chunks and offers Retry save without re-recording.
- [ ] Tray icon and window both show elapsed time while recording.
- [ ] Manual check on the laptop: a 30-minute call produces a playable Meeting.

## Comments
