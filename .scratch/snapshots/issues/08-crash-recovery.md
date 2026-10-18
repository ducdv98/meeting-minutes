# 08: Crash recovery

**Status:** ready-for-agent

**Blocked by:** 01 Take a Snapshot into the Meeting folder

**Parent:** `.scratch/snapshots/spec.md`

## What to build

Snapshots already written survive a crash, like Chunks do, and join the Meeting with the surviving Recording data. The remembered Capture region is lost, so the next Snapshot asks for a new region.

## Acceptance criteria

- [ ] After a simulated crash, written Snapshots are present in the restored Meeting at their positions
- [ ] A partly written Snapshot never appears as a broken image
- [ ] After a restart the next Snapshot asks for a region
- [ ] Core test simulates the crash using the existing recording test approach
