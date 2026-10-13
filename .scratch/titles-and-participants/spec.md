# Titles and Participants spec

Status: ready-for-agent

Let the user rename a Meeting at any time and name the people in it. Vocabulary: `CONTEXT.md` (**Title**, **Speaker label**, **Participant**). Folder naming: ADR 0009.

## Goal

The user renames a Meeting while it is still transcribing, names the remote side "Anna", and gets a Transcript and Summary that say "Anna" rather than "Remote".

## Requirements

### Rename at any time (ADR 0009)
- New Meeting folders, recorded or imported, are named by start time only: `2026-10-13 0930`, with ` (2)` added when two Meetings start in the same minute.
- The Title is stored only in `meeting.json`. A rename never moves the folder and never changes the Meeting ID.
- A rename works during Processing, Summary, Ask, playback and an active or unsaved Recording.
- Existing folders are not migrated. The Title inside `transcript.md` and `summary.md` is still updated on rename.

### Participants
- Speaker labels (Me / Remote / Unknown) stay as the app detects them. The user never edits them.
- Each Speaker label has a default Participant:
  - Me: the user's name from Settings, or "Me" when none is set, overridable per Meeting.
  - Remote: "Remote" until the user names it.
  - Unknown: "Unknown".
  - In a Voice note every Segment belongs to the user's Participant.
- The user can name a Speaker label's Participant for the whole Meeting.
- The user can move one Segment, or a selection of Segments, to any Participant in the Meeting, including a new one.
- Names are unique within a Meeting. Giving a Participant an existing name merges the two.
- Removing a Participant sends its Segments back to their Speaker label's default Participant.
- Participants belong to one Meeting; there is no Library-wide person record. The naming box suggests names used in other Meetings.
- Re-transcribing keeps the Participants:
  - Names given to whole Speaker labels carry over.
  - Each new Segment goes to the Participant who covered most of its time range in the previous Revision.
  - A new Segment with no clear majority falls back to its Speaker label's default Participant.
- Participant names are used everywhere:
  - the Transcript view and `transcript.md`;
  - Summary and Ask prompts, so Action item owners say "Anna";
  - Library search;
  - the Vocabulary hint for that Meeting only.
- An existing Summary or Answer is never rewritten. When Participants change after the Summary was generated, the Meeting view offers "Participants changed: regenerate Summary?". It never regenerates automatically.

## Out of scope

- Telling several remote voices apart automatically (diarization). That would need its own ADR.
- Migrating existing folder names.
- A shared list of people across Meetings.
