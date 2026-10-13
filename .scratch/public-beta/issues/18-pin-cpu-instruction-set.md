# Pin the CPU instruction set and thread count for release builds

Status: ready-for-agent
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
