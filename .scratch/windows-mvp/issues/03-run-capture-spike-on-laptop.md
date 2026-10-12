# Run the capture spike on the real laptop

Status: resolved
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

### 2026-10-12 — 8-minute headset run (resolved with accepted gaps)

Run: `capture-spike --minutes 10` (role `communications`), stopped early by hand at 8:00.478 (QPC). Logi USB Headset (Microphone and Speakers endpoints, both 48 kHz stereo float). The "call" was a Gemini voice conversation, not Zoom/Teams. Raw files are kept locally in `spikes/results/` (not committed).

Results:
- **Clean run.** 95,926 packets, 0 `error` rows, 0 `timestamp_error`, 0 arrival-time fallbacks. There was one discontinuity per Source, on its first packet only. The `stop` frame counts match the WAV data chunks exactly: mic 23,062,943 and system 23,063,104 frames.
- **Timeline.** WAV durations versus elapsed QPC are mic −0.01 ms and system +3.34 ms. System audio's first packet arrived at +1.432 s. Until then it was filled by 112 `idle_pad` rows and a 10,501-frame gap, so silence did advance the timeline. The spike dropped overlapping frames to keep each file on the clock: 386 mic frames (8.0 ms) and 31 system frames (0.65 ms).
- **Drift.** A linear fit of `device_position` against QPC gives both endpoints 48,000.798 Hz, which is +16.6 ppm versus QPC (about +60 ms/hour per Source). Mic versus system relative rate is −0.0014 ppm, so there is no measurable relative drift (same headset clock). No clap test was done, so the acoustic offset was not measured. Envelope cross-correlation found no usable shared event.
- **Separation (energy evidence).** In the 198 one-second bins where only the mic was active, `system.wav` was exactly zero in 197. When only system audio was active, the mic sat 21–24 dB below it, at its own noise floor. There is no measurable bleed either way. This was not confirmed by ear.
- **Levels.** The mic peaks at +2.17 dBFS (228 frames above |1.0|, float overrange). The mic channels are sample-identical, i.e. mono duplicated into stereo. There are no NaN or Inf samples.
- **Device switches.** None; no `device_switch` rows.

Accepted gaps (user decision, 2026-10-12): laptop spec not recorded; 8-minute Gemini call instead of 60-minute Zoom/Teams; no clap or acoustic drift measurement; headset plug/unplug not tested (deferred to ticket 07); 10-minute speaker echo/bleed test not done.

For ticket 05:
- Choose a drift policy for long recordings: frame drop/pad versus resampling against the +16.6 ppm device clock.
- Set a gain/clipping policy before FLAC/MP3 encoding, because mic floats exceed 1.0.
- Consider storing the mic as mono.
- Give `idle_pad`, `device_switch` and stop events QPC offsets and reasons. Log failed device reopen attempts too, not only successful switches.
- For 94% of loopback packets, the packet QPC is later than the arrival QPC. Do not substitute arrival time for packet time without checking.

