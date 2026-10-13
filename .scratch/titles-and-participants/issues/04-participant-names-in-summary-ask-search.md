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

## Comments

### 2026-10-13: implemented

- Summary and Ask lines are `[segment-id] <Participant>: text`; both system prompts say lines are `Participant: text` and name the user (the Meeting's Me Participant). Send plans and disclosures use the same lines. Ask retrieval also matches Participant names.
- Library search: the FTS `segments` table has an indexed `participant` column (index schema 4, rebuilt on first open). Participant changes re-index that Meeting; a Settings "Your name" change re-indexes every Meeting. The app opens the Library with "Your name", so a fresh index is built with it and startup does not re-index.
- Vocabulary: transcription adds the Meeting's real Participant names (including its Me Participant, never a Settings name the Meeting overrides) to the hint; saved Settings vocabulary is unchanged.
- Stale Summary: summary.json stores `participants_fingerprint` (the Me Participant plus each Segment's resolved name). The Summary panel offers "Participants changed: regenerate Summary?"; nothing regenerates on its own. Old summaries count as changed only when a Segment now resolves to a non-default name; a missing Revision counts as unchanged.
- Checks: core tests and clippy, `pnpm typecheck`, Vitest, Windows-gnu clippy of `src-tauri`.
- Review notes left as is: the cloud disclosure for long Meetings lists the map requests but not the later reduce requests (as before this ticket); a failed "Participants changed?" check hides the prompt rather than the Summary.
