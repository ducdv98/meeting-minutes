# Move Segments between Participants

Status: ready-for-human
Blocked by: 02
Spec: ../spec.md

## What

Let the user attribute single Segments to any Participant. This covers group calls and Imports.

- **Moving:** move one Segment, or a selection of Segments, to an existing Participant or a new one.
- **Removing:** remove a Participant; its Segments go back to their Speaker label's default Participant.
- **Re-transcribing:** a new Revision keeps the Participants.
  - Each new Segment goes to the Participant who covered most of its time range in the previous Revision.
  - With no clear majority it falls back to its Speaker label's default.
- **Allowed while busy:** moving Segments is allowed whenever editing Segment text is allowed.

## Acceptance criteria

- [ ] In an Import (all Unknown), the user assigns Segments to "Anna" and "Bao", and `transcript.md` reflects it.
- [ ] Multi-select moves several Segments in one action.
- [ ] Removing "Bao" returns Bao's Segments to their label's default Participant.
- [ ] After re-transcribing, Segments covering the same time as before keep their Participant (core test with overlapping old and new Segments, including a tie or no-majority case).
- [ ] Core and frontend tests cover moving, removing and carrying over.

## Comments

### 2026-10-13: implemented; laptop check pending

- `meeting.json` gains `"segment_participants": {"revision": N, "segments": {id: name}}`, used only for Revision N and holding only assignments that differ from the Segment's label Participant. `Participants` in `crates/core/src/participant.rs` is the single resolver (segment assignment, then label override, then default); `src/transcript.ts` mirrors it for the view.
- Library: `assign_segments` (current Revision only; refused for Voice notes), `rename_participant` (rename everywhere; onto an existing name merges), `remove_participant` (Segments go back to their label's default). Each re-renders `transcript.md`.
- Carry-over on re-transcription: from the Revision the moves were made on (not just N−1); only old Segments with the same Speaker label count, overlapping old Segments count their time once, and a Participant needs more than half of the new Segment's time.
- UI: Participants panel lists every Participant in use plus each present label's Participant (rename, merge, remove); Transcript Segments have checkboxes and a "Move to" bar with "New Participant…". Voice notes have neither moves nor checkboxes. After each change the view reloads the Meeting instead of re-applying the rules itself.
- Checks: core tests and clippy, `pnpm typecheck`, Vitest, Windows-gnu clippy of `src-tauri`.
- Review notes left as is: no dedicated Import test (the all-Unknown case is covered on a recorded Meeting); `Participants::new(...)` from a `MeetingFile` repeats in a few places.
- Laptop check: In an Import, tick several Segments and move them to a new Participant, rename it onto an existing name (merge), remove it, then Re-transcribe: the moves carry over to the new Revision.
