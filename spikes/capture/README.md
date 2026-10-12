# Capture spike

This CLI checks whether the Microphone and System audio Sources can be captured together, how far their output timelines drift, and whether capture survives a default device switch. It is a throwaway experiment, separate from the app.

On Windows, run `capture-spike --minutes 60 --out capture-test`. The folder contains `mic.wav`, `system.wav`, and `log.jsonl`. Both WAVs are 48 kHz stereo float. `--minutes` must be greater than zero and at most 180 to stay within the WAV size limit. Both Sources use the default Communications endpoint, which is typical for Teams or Zoom headset calls. Use `--role console` to select the default Console endpoints instead. The selected role is recorded in `start` and `device_format` rows and is used when polling for device switches.

Each `packet` row records the raw WASAPI `qpc_100ns`, `arrival_qpc_100ns`, `placed_qpc_100ns` used on the output timeline, device position, flags, inserted gap frames, dropped overlap frames, and the timestamp source. `timestamp_source: "arrival"` means the packet timestamp was invalid, implausible, or did not advance past the previous packet's timestamp. `idle_pad` marks silence added while no packets arrived. For output drift, divide each Source's frame count in `stop` by 48000 and compare it with `elapsed_qpc_100ns / 10000000`. Padding deliberately keeps this close to zero; compare packet device positions with QPC times to examine hardware clock drift. Inspect `device_switch` and `error` rows around a switch.

Download the `capture-spike-windows` artifact from the CI run's Artifacts section. It contains `capture-spike.exe`.
