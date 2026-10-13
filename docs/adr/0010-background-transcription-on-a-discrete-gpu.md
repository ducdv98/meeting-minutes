# Transcribe in the background during a Recording, only on a discrete GPU on AC power

After Stop, a one-hour Meeting took 6–7 minutes to transcribe on a Radeon 780M. That was the wait users noticed most. Now, while a Recording runs, completed Chunks are transcribed in the background, so after Stop only the last minute or so is left. We only do this when the transcription device is a discrete GPU and the PC is on AC power. Otherwise transcription starts after Stop, as before. The results are hidden during the Recording: this is not a live transcript, and the Draft appears only after Stop.

## Considered Options

- **Always transcribe after Stop, just faster**: a simpler model, but even on an RTX 4050 the user still waits minutes after every call.
- **Background transcription on every device**: on an integrated GPU or the CPU it competes with the video-call app for the same silicon, and on battery it drains the laptop during the call.
- **Show a live transcript during the Recording**: the user ruled it out. It would need partial, changing Segments during the call and a UI for them.

## Consequences

- Background results are a cache, not part of the Library. After a crash they are discarded, and the Meeting is transcribed from its Chunks. This keeps the Library as the only source of truth.
- If the PC is unplugged mid-Recording, background work stops and the rest is transcribed after Stop. Plugging back in resumes it.
- Segments must be stitched across Chunk edges, so that a sentence spoken across an edge becomes one Segment.
