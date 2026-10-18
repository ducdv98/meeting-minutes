# 07: Delete, undo and Meeting deletion

**Status:** ready-for-agent

**Blocked by:** 03 Snapshots in the Transcript

**Parent:** `.scratch/snapshots/spec.md`

## What to build

The user can delete a single Snapshot and undo the deletion, which restores its position and caption. Deleting a Meeting deletes its Snapshots.

## Acceptance criteria

- [x] Deleting a Snapshot removes it from the Transcript and the folder view
- [x] Undo restores the Snapshot with its position and caption
- [x] Deleting a Meeting leaves no Snapshot files behind
- [x] Tests cover delete, undo and Meeting deletion
