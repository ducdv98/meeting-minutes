# 01: Take a Snapshot into the Meeting folder

**Status:** ready-for-agent

**Blocked by:** None (can start immediately)

**Parent:** `.scratch/snapshots/spec.md`

## What to build

The core Snapshot store. During a Recording, a Snapshot can be taken through an injected screen-grabber (a fake in tests). It is encoded as PNG at native resolution, written durably into the Meeting's folder with its Timeline position and capture order, and recorded in an index that can be rebuilt from the folder alone. A Snapshot is refused when no Recording is running and for Imports. Follows ADR 0017 and the Snapshot terms in `CONTEXT.md`.

## Acceptance criteria

- [ ] A Snapshot taken during a Recording appears as a readable PNG with its Timeline position and order in the Meeting folder
- [ ] Deleting the index and rebuilding it from the folder alone restores every Snapshot
- [ ] A Snapshot is refused with no running Recording and for an Import
- [ ] The write is durable at the moment the Snapshot is taken
- [ ] Core tests in a new `snapshots` test file use a fake screen-grabber and the fake clock, following the existing recording tests
- [ ] Snapshots stay local: nothing is sent to any Provider
