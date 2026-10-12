# Scaffold Tauri v2 app and Windows CI

Status: ready-for-agent
Spec: ../spec.md

## What

Create the Tauri v2 app (Rust core + React/TypeScript UI) and a GitHub Actions workflow on `windows-latest` that runs Rust and UI tests and builds an unsigned NSIS installer as a build artifact. See ADR-0003.

## Acceptance criteria

- [ ] App launches on Windows 11 to an empty Library screen with a tray icon (Open, Quit).
- [ ] `cargo test` and UI tests run locally on Linux where possible, and in CI on Windows.
- [ ] CI uploads the installer artifact on every push.
- [ ] README explains how to build and how to install the CI artifact.

## Comments
