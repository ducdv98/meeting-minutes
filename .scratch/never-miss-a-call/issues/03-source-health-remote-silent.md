# 03: Source health: Remote silent

**What to build:** Extend Source health with a *remote silent* warning on System audio: System audio has stayed below the audible threshold for 3 minutes while the Microphone was at speech level for a meaningful share of that window. It appears, clears and is dismissed the same way as the no signal warning from ticket 02, with its own wording ("the other side may not be captured"). It never fires in a Voice note, never fires when both Sources are quiet, such as in a waiting room, and paused time and device switches behave as in ticket 02. See `.scratch/never-miss-a-call/spec.md`.

**Blocked by:** 02

Status: ready-for-agent

- [ ] Core tests: remote silent after 3 min of System audio silence with the Microphone at speech level; none when both are silent; none when the Microphone only carries low noise; none in a Voice note; paused time not counted; recovery clears it
- [ ] The Capture bar and the recording bar show the remote silent wording (UI tests)
- [ ] Dismiss and Open sound settings work as in ticket 02
