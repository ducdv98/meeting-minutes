# Pin the CPU instruction set and thread count for release builds

Status: ready-for-human
Blocked by: 17
Spec: ../spec.md

## What

whisper.cpp builds with `GGML_NATIVE` on when it isn't cross-compiling. So CI's Windows builds are tuned to the GitHub runner's CPU, not to users' PCs (ADR 0008). If the runner has AVX-512, the Beta can crash with an illegal instruction on most Intel laptops.

- Set `GGML_NATIVE=OFF` with `GGML_AVX=ON`, `GGML_AVX2=ON`, `GGML_FMA=ON`, `GGML_F16C=ON` and `GGML_BMI2=ON` (x86-64-v3) for CI and release builds. whisper-rs-sys passes `GGML_*` environment variables through to CMake; set them in the workflow env or `.cargo/config.toml` `[env]`.
- Threads: use the physical core count (capped at 8), not logical CPUs minus one. Hyperthreads slow ggml down.

## Acceptance criteria

- [ ] The CI and release workflows build whisper.cpp with the pinned flags. The build log (or the system info from ticket 17) shows `AVX2 = 1` and `AVX512 = 0`.
- [ ] Thread count equals physical cores (capped at 8). It is tested with a fake core count where practical.
- [ ] The installed build runs on the owner's laptop, and diagnostics show the pinned flags.

## Comments

### 2026-10-13: implemented; CI and laptop check pending

Codex implemented this ticket and Claude reviewed it.
- The Windows jobs in `ci.yml` and `release.yml` set `GGML_NATIVE=OFF`, AVX/AVX2/FMA/F16C/BMI2 on and AVX512 off. whisper-rs-sys passes these to CMake, and MSVC gets `/arch:AVX2`.
- The flags are not in `.cargo/config.toml`, because the aarch64 Linux dev box also builds whisper.cpp.
- Both jobs run `cargo test -p meeting_minutes_whisper pinned_cpu_instruction_set -- --ignored`. It checks that whisper.cpp's system info has `AVX2 = 1` and no `AVX512 = 1`.
- Threads are physical cores (sysinfo), capped at 8. When the physical count is unknown, it falls back to half the logical count.
- Still to do: the next CI run must pass the flag test, and the installed build's diagnostics on the laptop must show `AVX2 = 1`.
