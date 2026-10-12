# Capture spike: mic + system audio CLI

Status: ready-for-agent
Blocked by: 01
Spec: ../spec.md

## What

Build a throwaway Rust CLI (in the repo, e.g. `spikes/capture`) using the `wasapi` crate that records Microphone and System audio (endpoint loopback on the default render device) for N minutes into two separate WAV files, logs sample rates, packet gaps and timestamps, and keeps going across default-device changes, logging each switch. Not part of the app. Answers: does capture work, how much do the two Sources drift, does device switching work. See `docs/research/windows-audio-capture.md`.

## Acceptance criteria

- [ ] `capture-spike --minutes 60 --out <dir>` produces `mic.wav`, `system.wav`, `log.jsonl`.
- [ ] Log has device format, per-packet QPC timestamps, discontinuity flags and device-switch events.
- [ ] System-audio silence (nothing playing) still advances the timeline (no gaps in output).
- [ ] Built by CI as a Windows artifact.

## Comments
