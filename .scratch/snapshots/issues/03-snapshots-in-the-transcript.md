# 03: Snapshots in the Transcript

**Status:** ready-for-agent

**Blocked by:** 01 Take a Snapshot into the Meeting folder

**Parent:** `.scratch/snapshots/spec.md`

## What to build

The Meeting view shows each Snapshot as a thumbnail inline in the Transcript at its Timeline position, between the Segments around it. Clicking opens it at full size inside the app, with an "open folder" action. The Meeting view shows the total size of the Meeting's Snapshots. No separate gallery.

## Acceptance criteria

- [ ] A thumbnail renders between the correct Segments for its Timeline position
- [ ] Clicking a thumbnail opens a full-size in-app viewer
- [ ] The viewer has an "open folder" action that reveals the file
- [ ] The Meeting view shows the total Snapshot size
- [ ] UI tests with mocked IPC extend the Meeting view and Transcript view tests
