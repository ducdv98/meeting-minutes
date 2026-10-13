# Transcribe on the GPU with Vulkan, falling back to CPU

Status: ready-for-agent
Blocked by: 17, 18
Spec: ../spec.md

## What

Enable whisper-rs's `vulkan` feature on Windows builds (ADR 0008).

- Load the model with `use_gpu(true)` and `flash_attn(true)`.
- If no Vulkan device is found, or the GPU context fails to load, fall back to CPU and log the reason.
- The release and CI Windows jobs install the Vulkan SDK and set `VULKAN_SDK`.
- Check that the Linux core tests and the Windows-gnu clippy check with the stub whisper crate still work.

Expected effect: about 3–4× faster than CPU on recent integrated GPUs such as the Radeon 780M (whisper.cpp PR #3492), and more on discrete GPUs.

## Acceptance criteria

- [ ] On the owner's laptop (Ryzen 7 8745H, Radeon 780M), diagnostics show the GPU backend, and the RTF is at least 3× better than the CPU RTF from ticket 17 for the same model.
- [ ] On a machine or VM without a Vulkan GPU, transcription still completes on CPU and diagnostics say `CPU`.
- [ ] Transcript Segments match the CPU run's timing (same Meeting, same model; text may differ slightly).
- [ ] The installer doesn't ship the Vulkan SDK; it relies only on the GPU driver's `vulkan-1.dll`.
