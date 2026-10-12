# Meeting view with Timeline player

Status: ready-for-agent
Blocked by: 05
Spec: ../spec.md

## What

Meeting detail screen: title (renameable, which renames the folder), date, duration, language, a player for the mixed MP3 with Markers shown on the scrubber, and a seek API other views will use (seek to an ms position and play a span).

## Acceptance criteria

- [ ] Renaming a Meeting renames its folder safely and updates the index.
- [ ] Markers (pause, gap, device switch) show on the scrubber.
- [ ] `playSpan(start_ms, end_ms)` plays that span with a small lead-in.

## Comments
