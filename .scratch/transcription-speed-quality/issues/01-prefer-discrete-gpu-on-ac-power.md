# Prefer the discrete GPU on AC power

Status: ready-for-human
Spec: ../spec.md

## What

Today `plan_backend` (`crates/whisper/src/lib.rs`) picks the Vulkan device with the most memory. On the user's laptop that is the Radeon 780M, which reports shared system RAM, rather than the RTX 4050 (6 GB). Change the rule:

- Rank devices by type first: discrete GPU, then integrated GPU, then CPU. Within one type, keep the current rule (most memory, ties in ggml order). Use the device type that ggml/Vulkan reports (`VkDeviceInfo` or the Vulkan physical-device type), not the device name.
- On battery, skip discrete GPUs. Read the power state with `GetSystemPowerStatus`. Treat an unknown state as AC power.
- Decide when each transcription starts, not once at app launch.
- Copy diagnostics and `transcription_completed` include the chosen device name and the power state (for example `backend=Vulkan device="NVIDIA GeForce RTX 4050 Laptop GPU" power=ac`).
- Keep the CPU fallback and the existing `GpuUnavailable` and `GpuLoadFailed` events.

## Acceptance criteria

- [ ] Unit tests: given a discrete device with 6 GB and an integrated device with 16 GB, choose the discrete one on AC power and the integrated one on battery. With only discrete devices on battery, choose CPU. With an unknown power state, behave as on AC power.
- [ ] Existing tie and empty-list tests still pass.
- [ ] Diagnostics show the device and power state. The diagnostics parsing tests are updated.
- [ ] `main.rs` builds for Windows (see the memory note on Windows clippy).
- Laptop check: On AC power, transcribe a Meeting and Copy diagnostics: `transcription_completed` and the Transcription backend name the RTX 4050 with `power=ac`. Unplug and transcribe again: the Radeon 780M is used with `power=battery`.
