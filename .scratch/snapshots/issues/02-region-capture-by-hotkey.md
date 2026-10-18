# 02: Region capture by hotkey

**Status:** ready-for-agent

**Blocked by:** 01 Take a Snapshot into the Meeting folder

**Parent:** `.scratch/snapshots/spec.md`

## What to build

During a real Recording the user presses a global hotkey, drags a Capture region on a full-screen overlay, and a Snapshot is saved. Later presses of the first hotkey reuse the region (or draw one when none exists); the second hotkey always draws a new region. A brief on-screen confirmation shows after each Snapshot. The Capture region lasts for the Recording and is not saved to the Meeting. This ticket carries the riskiest native work, so prototype it first.

## Acceptance criteria

- [ ] The default hotkeys `Ctrl+Alt+S` (repeat or draw) and `Ctrl+Alt+Shift+S` (always new region) take a Snapshot while another app has focus
- [ ] The first Snapshot with no region asks for one; later ones reuse it until a new region is drawn
- [ ] The grabbed image matches the drawn region on a single monitor and on monitors with different scaling
- [ ] The region overlay and the confirmation never appear in a Snapshot or in screen sharing
- [ ] The Capture region is forgotten when the Recording ends
- [ ] Hotkeys are registered only while a Recording runs
- [ ] Manual checklist from the spec is run and the result noted in the ticket

## Manual checklist (2026-10-18)

Not run: this workspace is Linux and cannot launch the Windows native layer. On a Windows machine, verify a region drag on one display and on displays with different scaling; the first Snapshot prompting for a region; both repeat and redraw hotkeys while another app has focus; the overlay and confirmation absent from saved images and screen sharing; hotkey clash reporting; and a Snapshot while paused. The Capture bar check belongs to ticket 05.
