# 04: Snapshots while paused

**Status:** ready-for-agent

**Blocked by:** 01 Take a Snapshot into the Meeting folder; 03 Snapshots in the Transcript

**Parent:** `.scratch/snapshots/spec.md`

## What to build

A Snapshot can be taken while the Recording is paused, for example to show something private. It sits at the pause Marker's position on the Timeline so the Timeline does not change, and several Snapshots in one pause keep their capture order.

## Acceptance criteria

- [x] A Snapshot taken while paused is accepted and positioned at the pause Marker
- [x] Two Snapshots in one pause appear in capture order
- [x] The Timeline length and Markers are unchanged by Snapshots
- [x] Core test covers the position and order; a UI test shows them in the Transcript
