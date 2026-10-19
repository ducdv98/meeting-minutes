# 02: Source health: no signal on the Microphone

**What to build:** While a Recording runs, the recorder's status reports Source health for each Source. When the Microphone has delivered pure digital silence for 60 seconds, its health becomes a *no signal* warning. Pure digital silence means packets flagged silent by the device or all-zero samples, not just a low level. The Capture bar and the main window's recording bar show the warning with the Source and its device, plus **Open sound settings** (opens Windows Sound settings) and **Dismiss**. The warning clears by itself when the Microphone carries a signal again. Paused time doesn't count, and a device switch restarts the window. Dismissing applies to this Source in this Recording until it recovers and fails again; the dismissal is kept in the recorder so both views agree. The Recording never stops or pauses because of a warning, and the existing 3-second "Silent" tag is unchanged. See `.scratch/never-miss-a-call/spec.md`.

**Blocked by:** None (can start immediately)

Status: ready-for-agent

- [ ] Core tests (recorder seam, fake clock and opener): no signal after 60 s of digital silence; none after 60 s of low-level noise; paused time not counted; a device switch restarts the window; recovery clears the warning; a dismissed warning stays hidden until recovery and a new failure
- [ ] The Capture bar and the recording bar show the warning, Open sound settings and Dismiss (UI tests with mocked IPC)
- [ ] Open sound settings opens Windows Sound settings
- [ ] The Recording continues through a warning
- [ ] The existing "Silent" tag behaves as before
