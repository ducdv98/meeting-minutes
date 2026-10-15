# Tauri v2 with a Rust core

The app is a Tauri v2 desktop app: audio capture (WASAPI via the `wasapi` crate), Chunk writing, transcription (`whisper-rs` / whisper.cpp) and storage live in one Rust core, and the UI is React + TypeScript. Because we are building our own UI (ADR-0001), there is no existing Electron interface to reuse. Tauri keeps capture and inference in-process instead of in a separate helper program, ships a ~10 MB installer instead of ~150 MB, and leaves a macOS port possible later.

## Considered Options

- **Electron + separate Rust capture program**: larger, needs a link between two processes, mainly worth it to reuse an existing web UI.
- **.NET / WinUI 3 + NAudio**: most native on Windows and the quickest capture prototype, but Windows-only.

## Consequences

- Rust `wasapi` documents the per-process loopback `include_tree` flag backwards; v1 captures all system audio (endpoint loopback) and does not use per-process loopback.
