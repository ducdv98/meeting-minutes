# Background transcription during a Recording

Status: ready-for-agent
Spec: ../spec.md
Blocked by: 01, 06, 07, 08

## What

Implement ADR 0010:

- When a Recording starts and ticket 01's choice is a discrete GPU on AC power, start a background transcriber for that Meeting. It transcribes each Source's completed Chunks, in order, as they are written.
- Keep one loaded engine for the Recording, not one per Chunk. The model is the one Auto (or Settings) resolves.
- Language: use the Meeting language, or detect it from the first speech, then keep it for the Meeting.
- Stitch across Chunk edges: hold back words after the last sentence end of each processed span, and prepend them, with their audio, to the next span, so that a sentence spoken across an edge becomes one Segment.
- Nothing is shown during the Recording.
- After Stop, Processing:
  - reuses the background results;
  - shows them at once as the Draft (ticket 08);
  - transcribes only the remaining audio;
  - publishes one Revision.
- **Power change:** on a switch to battery, stop after the current span and keep the results. On a switch back to AC, resume.
- **Pause:** finish the captured audio, then idle.
- **Crash or app quit:** results live only in memory or in a temp cache outside the Meeting folder, and are discarded. The Meeting is then transcribed from its Chunks as usual.
- **Device conflict:** if a second Meeting is queued for Processing while a Recording runs, it waits for that Recording's background transcriber or uses another device. Do not run two models on one 6 GB GPU at once.

## Acceptance criteria

- [ ] Core tests with a fake engine:
  - Chunks are transcribed in order during a Recording;
  - a sentence across a Chunk edge becomes one Segment;
  - Stop transcribes only the remainder;
  - the result equals the result of after-Stop-only transcription, within the segmenter's rules.
- [ ] Power switch to battery stops background work, and the remainder is done after Stop. A switch back to AC resumes it.
- [ ] Simulated crash: no background state survives, and the Meeting transcribes normally after restart.
- [ ] On the RTX 4050, a one-hour Meeting shows the Draft within 5 s of Stop and publishes its Revision within about a minute (record in `## Comments`).
