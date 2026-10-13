# Name the Participant behind each Speaker label

Status: ready-for-human
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

## Comments

### 2026-10-13: implemented; laptop check pending

- A Participant is identified by its name. `meeting.json` stores per-Meeting overrides as `"participants": {"me": …, "remote": …, "unknown": …}`; an absent key means the default (Settings "Your name" or "Me", "Remote", "Unknown"). Giving two labels the same name merges them; setting a label back to its default removes the override (and so splits a merge).
- `crates/core/src/participant.rs` resolves names; `transcript.md` is re-rendered when a name changes, a Segment is edited, the Title changes and on transcription. A Settings name change does not rewrite existing files.
- Voice note = recorded with only the Microphone; `MeetingDetail.voice_note` carries it to the UI, which then shows a single Me Participant.
- Checks: core tests, `pnpm typecheck`, Vitest, Windows-gnu clippy of `src-tauri` with a whisper stub (caught a `map_err(Clone::clone)` compile error and an `unnecessary_unwrap` lint, both fixed).
- Left for later: `transcribe_meeting_named` / `render_markdown_named` wrappers could fold into the originals once the transcription-speed-quality work in `transcript.rs` / `processing.rs` lands; ticket 03 should replace the per-label inputs with a real Participant list.
- Laptop check: Set "Your name" in Settings, then name Remote in a recorded Meeting: the Transcript view and `transcript.md` show the names, and the Speaker label shows as a tooltip.
