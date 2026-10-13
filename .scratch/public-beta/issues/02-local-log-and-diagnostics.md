# Local log and Copy diagnostics

Status: ready-for-human
Blocked by: none
Spec: ../spec.md

## What

Add a rolling, size-capped log file in app data (Rust core and Tauri app). Log lifecycle events, errors, device changes, Processing steps, provider kind and HTTP status, never content. Add a command that builds a diagnostics text block (app version, Windows version, CPU/RAM, audio devices, selected model, provider kind, recent log lines) for Help → Copy diagnostics (the UI button lands in ticket 11).

## Acceptance criteria

- [x] Log rotates and stays under a fixed cap.
- [x] Unit test: Transcript text, Summary/Ask content and API keys never appear in log output or diagnostics.
- [ ] Diagnostics command returns the listed fields; works with no provider and no model.
- [x] No new network calls.

## Comments

- 2026-10-13: Added a 512 KiB rotating local log through `tauri-plugin-log`, fixed lifecycle event names, provider kind and HTTP status logging, a pure diagnostics builder with privacy tests, and `get_diagnostics` plus a typed frontend wrapper. The core test covers absent provider/model and rejects content-bearing log lines; the Windows command still needs a laptop check for OS, hardware, audio-device collection, and empty setup. Confirm the file rotates and the command returns recent events there. No network path was added. Fixed the diagnostics parser to accept the plugin's real `[date][time][target][level] message` format while dropping unknown or content-bearing lines.
