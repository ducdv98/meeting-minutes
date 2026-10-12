# Crash recovery and multi-Recording Meetings

Status: ready-for-agent
Blocked by: 05
Spec: ../spec.md

## What

On startup, find orphan Chunks from an interrupted Recording and recover them into a finalised Recording of their Meeting. Offer to continue the Meeting, which adds a new Recording to it. The Timeline joins Recordings in order with a gap Marker between them.

## Acceptance criteria

- [ ] Killing the app mid-recording loses at most the Chunk in progress; on relaunch the Meeting is recovered and playable.
- [ ] Continue adds Recording 2 to the same Meeting; the Timeline and mixed MP3 cover both with a gap Marker.
- [ ] Unit tests: Timeline maths across Recordings, pauses and gaps; recovery from truncated Chunks.

## Comments
