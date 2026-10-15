# A Meeting set to Auto-detect detects its language

Status: ready-for-agent

## Problem

An English Meeting was transcribed by Managed transcription as Vietnamese: its Transcript has `"language": "vi"` and `"language_detected": false`, while its `meeting.json` has `"language": null` and the app shows the Meeting as **Auto-detect**.

Cause: `meeting_language()` (`crates/core/src/processing.rs`) returns `request.default_language` (the Settings "Meetings are mostly in" value, wired in `src-tauri/src/main.rs` as `default_language: settings.language`) whenever the Meeting's language is null. So with a Settings default of Vietnamese, every Meeting the UI labels "Auto-detect" (App.tsx Meeting row, the Language select in MeetingView.tsx, LibraryTable) is forced to Vietnamese and never detected. Groq then produces translated nonsense of the English speech.

## Fix

Make the Meeting's own choice authoritative, so what the UI shows is what Processing does:

- When a Meeting is created (Recording start, Voice note, Import), store the Settings default language in its `meeting.json` `language`. If the Settings default is Auto-detect, store null.
- In Processing, a Meeting whose `language` is null is detected. Remove the fall-back to the Settings default in `meeting_language` (and drop `default_language` from `TranscribeRequest` if nothing else needs it).
- Background transcription during a Recording uses the same rule (check where `BackgroundTranscriber::new` gets its language).
- The Meeting's Language select keeps working: choosing Auto-detect stores null and the next Processing detects.
- Existing Meetings with null language become Auto-detect, which matches what the UI already shows for them.

## Acceptance criteria

- [ ] Regression test: a Meeting with `language: null` processed while the Settings default is Vietnamese produces a Transcript with the detected language and `language_detected: true` (use a fake engine whose detection returns English).
- [ ] A new Meeting created while the Settings default is Vietnamese has `"language": "vi"` in `meeting.json`; with Auto-detect it has null.
- [ ] Tests for Rust core and the frontend (`pnpm test` / vitest, excluding `.claude/worktrees`) pass.
