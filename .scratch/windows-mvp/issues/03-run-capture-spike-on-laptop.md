# Run the capture spike on the real laptop

Status: ready-for-human
Blocked by: 02
Spec: ../spec.md

## What

Run the capture spike CLI on the user's Windows laptop during real calls and record the results here. This is the gate for the capture design.

## Acceptance criteria

- [ ] Record the laptop spec: CPU, RAM, GPU/NPU, Windows build.
- [ ] 60-minute real call (Zoom or Teams) with a headset: both files play, remote voices are only in `system.wav`, your voice only in `mic.wav`.
- [ ] Measured drift between Sources after 60 minutes (e.g. a clap heard in both on speakers) is written down.
- [ ] Plug or unplug a headset mid-call: capture continues and the switch is logged.
- [ ] Repeat 10 minutes on speakers to observe echo/bleed.
- [ ] Results appended under `## Comments`.

## Comments
