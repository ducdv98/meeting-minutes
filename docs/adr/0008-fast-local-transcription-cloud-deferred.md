# Fast local transcription on the GPU; cloud transcription deferred

> Superseded by ADR 0011 for cloud transcription: it now runs on the user's own free Groq key. The local speed-ups here still apply to the local fallback.

Before the first release, local transcription was too slow: Medium on CPU took about 30 minutes for a 16-minute Meeting on a Ryzen 7 8745H. We considered a cloud transcription default, with an app-owned Cloudflare Workers AI or Groq key behind a rate-limited relay. We rejected it for now because of its ongoing cost and the backend it would need. Instead we speed up the local engine:
- a Vulkan GPU build of whisper.cpp, falling back to CPU;
- Large v3 turbo (`q5_0`) in place of Medium;
- Silero VAD to skip silence;
- a fixed CPU instruction set for release builds.

ADR 0002's local-first rule stands unchanged. The research behind this is in `docs/research/cloud-transcription.md`.

## Considered Options

- **App-paid cloud transcription by default** (Workers AI `whisper-large-v3-turbo` at about $0.03 per audio hour, behind a Worker with per-install quotas and a global daily cap):
  - About $17 a month for 100 users and $156 for 1,000, plus a backend to run and abuse to handle.
  - **Never ship a provider key inside the app.** It can be extracted, and Groq's terms hold the account owner responsible for it.
  - Revisit this if local speed on low-end laptops still blocks people, or once a paid plan exists.
- **CUDA**: NVIDIA only, needs the MSVC toolchain, and adds runtime DLLs to ship. Vulkan covers AMD, Intel and NVIDIA with one build.
- **OpenVINO**: whisper-rs has no feature for it, and it speeds up only the encoder.

## Consequences

- The release build machine needs the Vulkan SDK. End users only need their GPU driver.
- Speed now depends on the GPU. Copy diagnostics reports the backend and the real-time factor, so slow reports can be explained.
- Medium is removed from the model list. Settings that name it migrate to Large v3 turbo.
