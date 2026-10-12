# Crash recovery and multi-Recording Meetings

Status: ready-for-human
Blocked by: 05
Spec: ../spec.md

## What

On startup, find orphan Chunks from an interrupted Recording and recover them into a finalised Recording of their Meeting. Offer to continue the Meeting, which adds a new Recording to it. The Timeline joins Recordings in order with a gap Marker between them.

## Acceptance criteria

- [ ] Killing the app mid-recording loses at most the Chunk in progress; on relaunch the Meeting is recovered and playable.
- [ ] Continue adds Recording 2 to the same Meeting; the Timeline and mixed MP3 cover both with a gap Marker.
- [x] Unit tests: Timeline maths across Recordings, pauses and gaps; recovery from truncated Chunks.

## Comments

### 2026-10-12 — implemented; laptop check pending

Code is done and tested on Linux, and the Windows build passes `cargo clippy --target x86_64-pc-windows-gnu`. The design is in `../06-core-design.md`. Status is `ready-for-human` until the kill test is run on the laptop.

- Core (`crates/core`):
  - `timeline.rs`: `place_recording` puts a Recording after the earlier ones. It adds a `gap` Marker at the join, offsets pause Markers, and clamps them to the Recording's length.
  - `finalize_recording`:
    - Salvages truncated Chunks first. A `.part` is cut to whole frames and kept, so a kill loses only the unflushed tail of the Chunk in progress, not the whole Chunk.
    - Refuses Recording N until Recordings 1..N-1 are saved.
    - Rebuilds `meeting.mp3` across the whole Timeline. Earlier Recordings are decoded from their FLACs, each cut or padded to its `duration_ms`.
    - Keeps a `finalizing` journal in the Chunk dir, so files left by a save that was killed are removed on retry. Only files listed in the journal are ever deleted.
    - Saving an already-saved Recording just removes the leftover Chunk dir.
  - `Library::orphan_recordings` and `Library::begin_next_recording`. The second refuses while unsaved Chunks exist in that Meeting.
  - `Recorder::recover`, `continue_meeting`, `dismiss_continue`. Recovered saves show as "Recovering…"; when one succeeds, the Meeting is offered for Continue.
- App: at startup, orphan Chunks are queued and saved one after another. A failed recovery shows Retry save. The window offers "Continue meeting" or "Dismiss" for each recovered Meeting; Continue shows the Recording notice first if it was never acknowledged.
- This also covers ticket 05's leftover: failed saves are no longer lost on restart, because their Chunks are found as orphans.
- Known gaps (from review):
  - Renames are not followed by a directory fsync, so after a power loss (not a kill) the last completed Chunk or a fresh `recording.json` can go missing. std cannot fsync a directory on Windows, and NTFS journals metadata. A Chunk dir with no `recording.json` cannot be recovered and stays as a failed save.
  - When Recording 2+ is saved, `meeting.mp3` is replaced before `meeting.json` is written. If that write fails, the MP3 is ahead of `meeting.json` until Retry save rebuilds both.
  - `recover` queues one orphan per Meeting. The app cannot create two, because Continue refuses while Chunks are unsaved.
- Manual check still to do on the laptop:
  1. Start recording and kill the app from Task Manager after about 30 s.
  2. Relaunch. The Meeting should be recovered and play.
  3. Press Continue meeting, record a bit, then Stop.
  4. `meeting.json` should list 2 Recordings and a `gap` Marker, and `meeting.mp3` should cover both.
