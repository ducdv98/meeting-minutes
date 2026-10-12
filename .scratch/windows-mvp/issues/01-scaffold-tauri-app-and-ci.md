# Scaffold Tauri v2 app and Windows CI

Status: resolved
Spec: ../spec.md

## What

Create the Tauri v2 app (Rust core + React/TypeScript UI) and a GitHub Actions workflow on `windows-latest` that runs Rust and UI tests and builds an unsigned NSIS installer as a build artifact. See ADR-0003.

## Acceptance criteria

- [x] App launches on Windows 11 to an empty Library screen with a tray icon (Open, Quit).
- [x] `cargo test` and UI tests run locally on Linux where possible, and in CI on Windows.
- [x] CI uploads the installer artifact on every push.
- [x] README explains how to build and how to install the CI artifact.

## Comments

- 2026-10-12 (Claude): Implemented. Cargo workspace splits a pure-Rust `crates/core` (tested on Linux) from `src-tauri` (needs WebView, built only on Windows CI). Linux checks pass: core test, fmt, typecheck, 3 UI tests, `pnpm build`. Still to verify on Windows: the first CI run on `windows-latest` (Tauri crate compile, NSIS artifact) and launching the installed app on Windows 11 (empty Library, tray Open/Quit, closing the window hides it to the tray).
- 2026-10-12 (Claude): Verified. CI run 38202678565 passed on `windows-latest` and uploaded the NSIS installer. On Windows 11 the user confirmed: empty Library, tray Open/Quit, and close hides to tray. Before closing, switched the icons and header to the brand pack in `assets/images/` (ADR-0004). Resolved.
