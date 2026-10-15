# 01: Tray click opens the app

**What to build:** A left-click or double-click on the tray icon opens and focuses the main window at once, restoring it if it is minimised or hidden. A right-click shows the existing tray menu. Today the tray has a menu but no click handler (`src-tauri/src/main.rs`, `TrayIconBuilder`), so a left-click opens the menu instead of the app. See `.scratch/v1-launch/spec.md`.

**Blocked by:** None

Status: ready-for-agent

- [x] Left-click on the tray icon shows, unminimises and focuses the window
- [x] Double-click does the same
- [x] Right-click shows the tray menu with Open, Record, Pause, Stop and Quit
- [ ] The window opens without a visible delay after the click
- [x] `cargo clippy --target x86_64-pc-windows-gnu -p meeting-minutes -- -D warnings` passes with the whisper crate stubbed (main.rs does not build on Linux otherwise)
