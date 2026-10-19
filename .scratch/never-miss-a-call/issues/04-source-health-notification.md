# 04: Source health Windows notification

**What to build:** When a Source starts a Source health warning that isn't dismissed, Windows shows one notification naming the Source and its device. This lets the user notice it while the Capture bar is collapsed or the main window is hidden. Clicking the notification opens and focuses the main window. A warning that clears and comes back later notifies again; a warning that stays on doesn't notify twice. This adds a notification plugin to the Tauri app. See `.scratch/never-miss-a-call/spec.md`.

**Blocked by:** 02

Status: ready-for-agent

- [ ] One notification each time a Source starts a warning, none for a dismissed or continuing warning
- [ ] Clicking the notification opens and focuses the main window
- [ ] The notification works with the main window hidden in the tray
- [ ] Windows build and clippy pass
