# Local log and Copy diagnostics

Status: ready-for-agent
Blocked by: none
Spec: ../spec.md

## What

Add a rolling, size-capped log file in app data (Rust core and Tauri app). Log lifecycle events, errors, device changes, Processing steps, provider kind and HTTP status, never content. Add a command that builds a diagnostics text block (app version, Windows version, CPU/RAM, audio devices, selected model, provider kind, recent log lines) for Help → Copy diagnostics (the UI button lands in ticket 11).

## Acceptance criteria

- [ ] Log rotates and stays under a fixed cap.
- [ ] Unit test: Transcript text, Summary/Ask content and API keys never appear in log output or diagnostics.
- [ ] Diagnostics command returns the listed fields; works with no provider and no model.
- [ ] No new network calls.
