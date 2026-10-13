# Transcribe on the GPU with Vulkan, falling back to CPU

Status: ready-for-human
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
- [ ] The installer doesn't ship the Vulkan SDK. It ships only the Khronos Vulkan loader (`vulkan-1.dll`) next to the exe, so the app still starts on PCs without a Vulkan runtime (a VM with a basic display adapter) and falls back to CPU.

## Comments

### 2026-10-13: implemented; Windows checks pending

- Windows builds enable whisper-rs Vulkan. A fallible Vulkan instance check avoids whisper.cpp's uncaught C++ exception when no driver is installed. The adapter selects the Vulkan device with the most reported memory, uses GPU and flash attention, and falls back to CPU with a content-free `gpu_unavailable` log event when no device is available or GPU context loading fails. Copy diagnostics probes the device without loading a model and reports its name; after a failed GPU load, it reports CPU.
- CI and release install Vulkan SDK 1.4.309.0 using `jakoch/install-vulkan-sdk-action@v1.7.0`. The `scripts/copy-vulkan-loader.ps1` step checks for `glslc.exe` and copies only `vulkan-1.dll` to `src-tauri/`. Tauri's resource map places it at the NSIS install root beside the exe.
- For a local Windows build, install Vulkan SDK 1.4.309.0, make sure `VULKAN_SDK` points to it, run `powershell -File scripts/copy-vulkan-loader.ps1` from the repo root, then run `pnpm tauri build --bundles nsis --config src-tauri/tauri.vulkan.conf.json`. The copied DLL is gitignored. The loader is only bundled through that extra config, so `pnpm tauri dev` and plain builds work without it (they use the GPU driver's `vulkan-1.dll`).
- Linux core and Whisper tests and clippy pass; the Windows-only Vulkan preflight type-checks in isolation for `x86_64-pc-windows-gnu`. Full Windows compilation, the NSIS layout, GPU RTF, CPU fallback in a no-GPU VM, and Segment timing still need checks on Windows machines.
- Tauri resource mapping: https://v2.tauri.app/develop/resources/ ; Windows resource directory: https://docs.rs/tauri/2.12.3/tauri/path/struct.PathResolver.html#method.resource_dir ; Windows installer: https://v2.tauri.app/distribute/windows-installer/ .
- Review fixes (Claude):
  - **Vulkan registration:** whisper.cpp#3750 reports that in static MSVC builds (whisper-rs-sys 0.15) the Vulkan backend can fail to register during static init, so ggml silently loads on CPU. The adapter now registers it again after `list_devices()`, the workaround from that issue. It reports `Vulkan` only when the backend is registered.
  - **Fallback:** a failed GPU load logs `gpu_load_failed` and falls back to CPU for that load only, with no process-wide latch. No device, or no registration, logs `gpu_unavailable`.
  - **Runner:** the Windows jobs are pinned to `windows-2022`. `windows-latest` is now `windows-2025-vs2026`, where whisper.cpp#3973 reports the Vulkan shader-generator build failing with MSVC C1041.
  - **Loader resource:** it moved to `src-tauri/tauri.vulkan.conf.json`, passed with `--config` in CI and release. A plain `tauri dev` or build doesn't need the copied DLL.
  - **Known limit:** a Vulkan driver that throws a C++ exception inside ggml after the `ash` preflight passes would still abort. That needs a C++ exception boundary in whisper.cpp/whisper-rs, which is out of scope here.
- **Still to verify:** the first CI run on `windows-2022` with the Vulkan build, the NSIS layout (`vulkan-1.dll` beside the exe), the laptop RTF with diagnostics showing `Vulkan (AMD Radeon 780M Graphics)`, and the CPU fallback in a VM with no GPU.
