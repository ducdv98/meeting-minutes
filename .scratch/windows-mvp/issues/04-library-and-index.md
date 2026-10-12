# Library folders and rebuildable index

Status: ready-for-agent
Blocked by: 01
Spec: ../spec.md

## What

Implement the Library: Meeting folder layout and `meeting.json` as in the spec, collision-safe naming, and the SQLite index in app data that can be rebuilt from the folders. The UI lists Meetings by date and title. No audio yet: create Meetings via a dev command or fixture.

## Acceptance criteria

- [ ] Meeting folder name `<YYYY-MM-DD HHmm> <title>`; collisions get a suffix; nothing is ever overwritten.
- [ ] Deleting the index and restarting rebuilds the same Meeting list from the folders.
- [ ] A hand-edited or added Meeting folder is picked up on rescan; malformed folders are reported, not crashing.
- [ ] Unit tests cover naming, collision and rebuild.

## Comments
