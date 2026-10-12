# Capture spike: mic + system audio CLI

Status: resolved
Blocked by: 01
Spec: ../spec.md

## What

Build a throwaway Rust CLI (in the repo, e.g. `spikes/capture`) using the `wasapi` crate that records Microphone and System audio (endpoint loopback on the default render device) for N minutes into two separate WAV files, logs sample rates, packet gaps and timestamps, and keeps going across default-device changes, logging each switch. Not part of the app. Answers: does capture work, how much do the two Sources drift, does device switching work. See `docs/research/windows-audio-capture.md`.

## Acceptance criteria

- [x] `capture-spike --minutes 60 --out <dir>` produces `mic.wav`, `system.wav`, `log.jsonl`.
- [x] Log has device format, per-packet QPC timestamps, discontinuity flags and device-switch events.
- [x] System-audio silence (nothing playing) still advances the timeline (no gaps in output).
- [x] Built by CI as a Windows artifact.

## Comments

Implemented in `spikes/capture` (see its README). Built and unit-tested on Linux, plus a `cargo check`/clippy of the Windows code for `x86_64-pc-windows-gnu`. The WASAPI capture itself has not run on Windows yet; ticket 03 covers that. Options: `--role communications|console` (default `communications`, the endpoint Teams/Zoom normally use with a headset); `--minutes` is capped at 180 (WAV 4 GB limit). Both WAVs are 48 kHz stereo float (shared mode with autoconvert). Output timelines are kept aligned to QPC by gap/idle padding, so measure hardware drift from packet `device_position` vs `qpc_100ns`, or with the clap test.
