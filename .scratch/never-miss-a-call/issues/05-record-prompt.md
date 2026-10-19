# 05: Record prompt: offer to record when a call starts

**What to build:** A pure Record prompt decider in core, fed on each check by the Microphone usage list from ticket 01, whether a Recording is active and the prompt settings. It decides when to show or close a prompt:

- It prompts once when an app starts using the Microphone.
- It doesn't prompt again while that app keeps using it, and prompts again when that app stops and starts again.
- It prompts only for a second app that starts while another is in use.
- It never prompts while a Recording is active, nor later in that same Microphone session.
- Apps already using the Microphone at the first check after launch never prompt.
- It closes an open prompt when its app stops using the Microphone or a Recording starts.

The Tauri layer polls about every 2 seconds and shows a small always-on-top prompt window near the tray corner. The window names the app, offers **Record** and **Not now**, closes by itself after about 20 seconds as Not now, and is excluded from screen sharing and Snapshots like the Capture bar. Record uses the same start command as the main window. If the Recording notice hasn't been acknowledged, it opens the main window on the notice instead. The prompt never starts a Recording by itself, and it works while the main window is hidden. See `.scratch/never-miss-a-call/spec.md`.

**Blocked by:** 01

Status: ready-for-agent

- [ ] Core decider tests cover every rule listed above
- [ ] The prompt window shows the app name, Record and Not now, and closes itself after about 20 s (UI test with mocked IPC)
- [ ] Record starts a Recording as the main window does, or opens the Recording notice first
- [ ] The prompt is excluded from screen sharing and Snapshots
- [ ] Windows build and clippy pass
