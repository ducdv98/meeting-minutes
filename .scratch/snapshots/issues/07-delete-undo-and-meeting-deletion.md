# 07: Delete, undo and Meeting deletion

**Status:** ready-for-agent

**Blocked by:** 03 Snapshots in the Transcript

**Parent:** `.scratch/snapshots/spec.md`

## What to build

The user can delete a single Snapshot and undo the deletion, which restores its position and caption. Deleting a Meeting deletes its Snapshots.

## Acceptance criteria

- [ ] Deleting a Snapshot removes it from the Transcript and the folder view
- [ ] Undo restores the Snapshot with its position and caption
- [ ] Deleting a Meeting leaves no Snapshot files behind
- [ ] Tests cover delete, undo and Meeting deletion
