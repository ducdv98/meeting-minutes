# Use Participant names in Summary, Ask, search and Vocabulary

Status: ready-for-agent
Blocked by: 02
Spec: ../spec.md

## What

Give the models and search Participant names instead of Speaker labels.

- **Prompts:** the Summary and Ask prompts (`summary.rs`, `ask.rs`) write transcript lines as `[segment-id] <Participant>: text`. Replace the hard-coded "Speaker is Me, Remote or Unknown" wording; name which Participant is the user.
- **Search:** Library search matches Participant names, so "Anna" finds Meetings where Anna is a Participant.
- **Vocabulary:** transcription adds the Meeting's Participant names to the Vocabulary hint for that Meeting only. The user's global Vocabulary is unchanged.
- **Stale Summary:** record which Participants a Summary was generated with. When they have changed since, the Meeting view offers "Participants changed: regenerate Summary?". Never regenerate automatically. Existing Summaries and Answers are not rewritten.

## Acceptance criteria

- [ ] With Remote named "Anna", a generated Summary can name Anna as an Action item owner (prompt-building unit test shows Participant names).
- [ ] An Ask sees Participant names in its context.
- [ ] Library search for a Participant name lands on that Meeting.
- [ ] Re-transcribing a Meeting with Participant "Nguyễn" passes "Nguyễn" in the Vocabulary hint; global Vocabulary in Settings is unchanged.
- [ ] Renaming a Participant after a Summary exists shows the regenerate prompt; nothing regenerates on its own.
