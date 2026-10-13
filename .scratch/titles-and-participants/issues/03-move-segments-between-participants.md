# Move Segments between Participants

Status: ready-for-agent
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
