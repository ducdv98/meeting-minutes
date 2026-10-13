# Transcribe each Source separately

Status: ready-for-agent
Spec: ../spec.md
Blocked by: 01

## What

Today `prepare_recording` mixes the Microphone and System audio into one 16 kHz stream, and `speaker_label` guesses Me or Remote from energy. Instead:

- `prepare_recording` produces one 16 kHz stream per Source that is present, each with its own timeline offset.
- `transcribe_meeting` transcribes each stream (with VAD) and labels its Segments by Source: Microphone → Me, System audio → Remote. Imports stay one stream labelled Unknown. A Voice note is Me.
- Run the two streams in parallel when the engine allows it (two `WhisperState`s on one context). Fall back to sequential if creating a second state fails. Measure both on ticket 04's machine.
- Merge both Sources' Segments in time order. Overlapping Segments from different Sources are allowed.
- Progress counts both streams.
- Language detection runs once per Meeting, on the first stream that has speech.
- Re-transcribing an old Meeting uses this path, and Participants carry over as in the Titles and Participants spec (majority time overlap).
- Remove the energy-based `speaker_label` only once no caller needs it.

## Acceptance criteria

- [ ] Core tests with synthetic two-Source audio: mic-only speech gives Me, system-only speech gives Remote, simultaneous speech gives one Segment per Source, and none are Unknown.
- [ ] An Import still gives Unknown. A Voice note gives Me.
- [ ] Timeline placement across multiple Recordings is unchanged (the existing tests pass).
- [ ] `transcription_completed` reports the summed audio and VAD durations for both streams.
