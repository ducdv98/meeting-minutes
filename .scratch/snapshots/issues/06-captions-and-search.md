# 06: Captions and search

**Status:** ready-for-agent

**Blocked by:** 03 Snapshots in the Transcript

**Parent:** `.scratch/snapshots/spec.md`

## What to build

The user adds an optional caption to a Snapshot after the meeting and can edit it later. The caption is stored beside the image in the Meeting folder, and Library search finds it and lands on the nearest Segment. A caption is not a Citation.

## Acceptance criteria

- [x] A caption can be added and edited in the Snapshot viewer
- [x] The caption is stored beside the image and survives an index rebuild
- [x] Library search by caption text lands on the nearest Segment
- [x] Summary and Ask do not read captions or images
- [x] Core and UI tests cover caption storage, edit and search
