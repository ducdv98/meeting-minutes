# Name the Participant behind each Speaker label

Status: ready-for-agent
Spec: ../spec.md

## What

Introduce Participants with the simplest form of naming: one Participant per Speaker label for the whole Meeting.

- **Settings:** add "Your name", used as the default Participant for Me. Blank means "Me".
- **Storage:** store each Meeting's Participants in the Meeting folder, not in the Transcript Revision, so they survive re-transcription. Keep the detected `speaker` on each Segment unchanged.
- **Naming:** in the Meeting view, the user can rename the Participant for Me (a per-Meeting override), Remote and Unknown.
  - Names are unique within the Meeting. Renaming to an existing name merges the two.
  - Suggest names already used in other Meetings.
- **Voice notes:** the user's Participant covers every Segment.
- **Display:** the Transcript view and `transcript.md` show Participant names in place of "Me", "Remote" and "Unknown".
  - Keep the Unknown styling for Segments still on the default Unknown Participant.
  - Show the detected Speaker label as a tooltip.

## Acceptance criteria

- [ ] With "Your name" set to "Duc", a new Meeting shows "Duc" for Me Segments; a per-Meeting override shows only in that Meeting.
- [ ] Renaming Remote to "Anna" updates every Remote Segment in the view and in `transcript.md`.
- [ ] Renaming Unknown to "Anna" when Remote is already "Anna" merges them.
- [ ] A Voice note shows the user's name on every Segment.
- [ ] Participant names survive closing and reopening the app, and a Library rescan.
- [ ] Core and frontend tests cover the default Participants, renaming, merging and the Voice note rule.
