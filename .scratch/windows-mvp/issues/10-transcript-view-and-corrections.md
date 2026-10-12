# Transcript view, edits and re-transcribe

Status: ready-for-agent
Blocked by: 09
Spec: ../spec.md

## What

Show the Transcript beside the player: click a Segment to play it; current Segment highlights during playback. Edit a Segment's text in place (keeps ID and Revision). Re-transcribe with another model or language creates a new Revision and makes it current; old Revisions are kept. Vocabulary editor in settings.

## Acceptance criteria

- [ ] Clicking a Segment plays its span.
- [ ] Edits persist to the current Revision's file and to `transcript.md`.
- [ ] Re-transcribe creates `transcript.r2.json`; earlier Citations still resolve against r1.

## Comments
