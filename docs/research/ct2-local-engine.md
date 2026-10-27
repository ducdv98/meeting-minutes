# CTranslate2 local engine spike

Status: **Windows Large v3 turbo measurement pending** (2026-10-27). This is a disposable benchmark in [`spikes/ct2/`](../../spikes/ct2/), outside the app workspace. No product integration decision has been made.

## Decision gate

The target machine is the slow Windows laptop with a Radeon 780M. Compare CT2 **CPU int8** against the app's **Vulkan whisper.cpp** path on the same Large v3 turbo corpus, with the same Silero VAD. The spike passes only if CT2's aggregate RTF is at most half the Vulkan RTF and corpus WER is no worse in both English and Vietnamese. The Windows measurements and MSVC build check require the user. Linux build success alone does not satisfy this gate. If the gate fails, stop. If it passes, create integration tickets and an ADR before touching the app.

### Radeon 780M Windows benchmark: pending user run

| Model | Backend / device | Set | WER | RTF | After VAD | Median s | p10 s | p90 s | <1 s | >20 s |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Large v3 turbo q5_0 | Vulkan Radeon 780M | English short | pending | pending | pending | pending | pending | pending | pending | pending |
| Large v3 turbo int8 | CT2 CPU | English short | pending | pending | pending | pending | pending | pending | pending | pending |
| Large v3 turbo q5_0 | Vulkan Radeon 780M | Vietnamese short | pending | pending | pending | pending | pending | pending | pending | pending |
| Large v3 turbo int8 | CT2 CPU | Vietnamese short | pending | pending | pending | pending | pending | pending | pending | pending |
| Large v3 turbo q5_0 | Vulkan Radeon 780M | English stitched | pending | pending | pending | pending | pending | pending | pending | pending |
| Large v3 turbo int8 | CT2 CPU | English stitched | pending | pending | pending | pending | pending | pending | pending | pending |

Use the benchmark's Markdown output to replace this table. The runner sums word errors and reference words across each short language set. RTF includes the engine call and VAD, divided by the **original** audio duration. Import and preparation are excluded. Segment statistics come from the final Meeting Transcript after the shared placement and sentence rules.

## Prototype behavior

- `Ct2Engine` implements `SpeechEngine` and requests `ct2rs` 0.10.2 with the `whisper` and `ruy` features. Its `Config` selects CPU, `INT8`, and the same 1–8 physical-core thread policy as `WhisperEngine`. It rejects models with a sample rate other than 16 kHz.
- CT2 uses `beam_size = 1` for greedy decoding, matching the benchmark's whisper.cpp greedy strategy. The runtimes still have distinct token suppression and long-form decoding implementations; WER is an empirical check, not assumed equivalent.
- Both engines use `WhisperSpeechDetector`, the same Silero model and default VAD parameters. The CT2 spike copies whisper.cpp's speech buffer policy: 100 ms overlap, 100 ms gap, and a time map back to original audio. CT2 returns `RawSegment` and `RawWord` ranges in milliseconds. It discards invalid word alignments the same way as `WhisperEngine`.
- CT2's [`generate_segments`](https://docs.rs/ct2rs/0.10.2/ct2rs/struct.Whisper.html#method.generate_segments) currently emits one segment per model audio chunk, roughly 30 seconds, with optional aligned words. The app's existing sentence placement may split those further, but segment distributions may still differ for reasons unrelated to ASR accuracy. Its high-level API also has no initial prompt or exposed detected-language result. This spike requires explicit clip languages and an empty vocabulary; it errors on a nonempty prompt. Cancellation is checked before and after inference, with no mid-call abort or intermediate progress.
- The benchmark requires a VAD path. If VAD is unavailable for either engine, it fails instead of silently comparing VAD against no VAD. The production `WhisperEngine` normally falls back to full audio on a VAD error, so this is a deliberate benchmark-only difference.

## Windows MSVC run for the user

Install Rust with the `x86_64-pc-windows-msvc` target, Visual Studio Build Tools with Desktop development with C++ (including MSVC and a Windows SDK), CMake, the Vulkan SDK with `VULKAN_SDK` set (for the whisper.cpp baseline), Python 3, FFmpeg, and `curl`. Open **x64 Native Tools Command Prompt for VS** or a PowerShell session with the MSVC environment loaded. The [`ct2rs` build script](https://docs.rs/crate/ct2rs/0.10.2/source/build.rs) builds CTranslate2 with CMake and static libraries; it warns that a static CRT flag may be needed. Run from the repository root:

```powershell
python scripts/benchmark-transcription download
$env:RUSTFLAGS = '-C target-feature=+crt-static'
$env:CMAKE_PARALLEL = '4'
cargo build --release --manifest-path spikes/ct2/Cargo.toml
python scripts/benchmark-transcription run `
  --model large-v3-turbo-q5_0=C:\models\ggml-large-v3-turbo-q5_0.bin `
  --device 0 `
  --ct2-model C:\models\whisper-large-v3-turbo-int8-ct2 `
  --vad C:\models\ggml-silero-v6.2.0.bin
```

First run `python scripts/benchmark-transcription list-devices` to find the Radeon Vulkan ID, then replace `0` above with that ID. A missing Vulkan backend fails the requested-device check. Repeat under the same power state and record CPU, RAM, executable size, and any build or antivirus events. The benchmark executable writes temporary Meetings and prints the table; copy its output here. An optional NVIDIA run needs a separate CUDA-enabled CT2 build and is outside this CPU gate.

The CT2 model directory must contain `model.bin`, `config.json`, `vocabulary.json`, `tokenizer.json`, and `preprocessor_config.json`. A preconverted [Large v3 turbo int8 CTranslate2 model](https://huggingface.co/Zoont/faster-whisper-large-v3-turbo-int8-ct2/tree/8d03bef2d94576dda118069ca1da6c721d27666f) has these files. Keep model files in `benchmarks/data/` or another untracked directory. Use the app's [ggml Large v3 turbo q5_0 file](../transcription-benchmark.md) for the Vulkan baseline. The two paths must use the same Large v3 turbo architecture; their quantization differs by design.

## Build and size findings so far

| Item | Finding |
|---|---|
| Linux native build | passed on aarch64 Linux: `cargo test --manifest-path spikes/ct2/Cargo.toml` built the crate and benchmark binary; 2 mapping tests passed |
| Linux runtime smoke | passed with a [tiny int8 CT2 model](https://huggingface.co/Kofi24/asr-whisper-tiny-ct2-int8/tree/005d8b7), the same Silero VAD, and whisper.cpp tiny on the JFK sample: both produced a transcript and reported 8.4 s after VAD. This checks the runtime path, not the Large v3 turbo corpus or Windows win condition. |
| Windows MSVC build | pending user run; do not infer it from Linux |
| CT2 int8 weights | [`model.bin` is 814 MB](https://huggingface.co/Zoont/faster-whisper-large-v3-turbo-int8-ct2/blob/8d03bef2d94576dda118069ca1da6c721d27666f/model.bin); repository total is about 818 MB including metadata and tokenizer |
| Existing ggml q5_0 weights | 574,041,195 bytes per `crates/core/src/models.rs` |
| Additional downloaded weight bytes | roughly 818 MB on a machine that keeps both; exact installed delta depends on model retention and packaging |
| Installer size delta | pending a Windows release build and packaging comparison; no installer was changed in this spike |
| Win condition | pending Windows RTF and WER |

The Large v3 turbo weights and FLEURS corpus were not available locally for a Linux corpus run. The user-run Windows table is the required evidence for the decision.

## Open risks

- **Word timestamps:** CT2's alignment output and the original-time mapping need corpus inspection, especially around VAD cuts. Word timings affect the app's sentence and citation placement. The 30-second chunk strategy may split words at chunk boundaries.
- **Segment rules:** `generate_segments` does not mirror whisper.cpp's segmentation. Compare final Segment duration tails and stitched-clip text, not only WER.
- **Long-form memory and control:** `ct2rs` stacks audio chunks for one generation and alignment call. The stitched 10-minute clip may use substantially more memory, and the current API offers no mid-call cancellation or progress callback.
- **Windows distribution:** The MSVC link may require static CRT; CMake and the C++ compiler must agree on architecture and runtime. A larger native executable or model download may affect antivirus scanning. Measure this on the target laptop before any shipping decision.
- **Licences:** [`ct2rs`](https://github.com/jkawamoto/ctranslate2-rs) and [CTranslate2](https://github.com/OpenNMT/CTranslate2) are MIT; the enabled ruy library is Apache 2.0 according to the ct2rs dependency notice. The chosen converted [model card](https://huggingface.co/Zoont/faster-whisper-large-v3-turbo-int8-ct2) says MIT. Verify all notices and model provenance before redistribution.
