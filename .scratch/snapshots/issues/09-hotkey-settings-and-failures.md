# 09: Hotkey settings and failures

**Status:** ready-for-agent

**Blocked by:** 02 Region capture by hotkey

**Parent:** `.scratch/snapshots/spec.md`

## What to build

The user can change both Snapshot hotkeys in Settings, next to the existing recording hotkeys. If a hotkey cannot be registered, the user is told so instead of it failing silently.

## Acceptance criteria

- [ ] Both hotkeys are configurable in Settings and the change takes effect on the next Recording
- [ ] A hotkey that clashes with another app shows a visible message naming it
- [x] The two Snapshot hotkeys cannot be set to the same combination
- [x] Settings UI tests cover change and registration failure
