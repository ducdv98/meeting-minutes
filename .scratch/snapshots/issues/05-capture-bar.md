# 05: Capture bar

**Status:** ready-for-agent

**Blocked by:** 02 Region capture by hotkey

**Parent:** `.scratch/snapshots/spec.md`

## What to build

A small floating bar shown only while a Recording runs, with pause/resume, stop, Snapshot and the elapsed time. It is always on top, draggable and collapsible. Starting a Recording stays in the main window. The bar reuses the existing recording state and commands and is excluded from capture and screen sharing.

## Acceptance criteria

- [ ] The bar appears when a Recording starts and disappears when it ends
- [ ] Pause/resume, stop and Snapshot work from the bar and match the main window state
- [ ] The bar can be dragged and collapsed
- [ ] The bar never appears in a Snapshot or when the whole screen is shared in a real meeting app
- [ ] The bar's Snapshot button follows the same repeat-or-draw behaviour as the first hotkey
