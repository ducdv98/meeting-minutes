# Cloud transcription skips audio with no speech

Status: ready-for-agent

## Problem

Microphone room noise becomes text such as "Thank you.", "so" or "Các bạn hãy đăng ký kênh để ủng hộ kênh của mình nhé." when a cloud Provider (Managed transcription, Groq, Gemini) transcribes it. Local Whisper does not do this because it runs Silero VAD (`WhisperEngine::speech_only`, `crates/whisper/src/lib.rs`) and sends only speech. The cloud branch of `EngineChain` sends every sample:

- `EngineChain::has_speech` (`crates/core/src/processing.rs`) returns true when any sample exceeds 0.01. Room noise passes (108 of 480,000 samples in the fixture).
- `EngineChain::transcribe` splits all audio with `cloud_span_bounds` and uploads every span, including silent stretches.
- `BackgroundTranscriber::transcribe_ready` (`crates/core/src/background.rs`) uses the same `has_speech` and then uploads 30 s spans.

Reproduced against Groq with the gateway's exact form fields (whisper-large-v3, verbose_json): 16 s and 30 s of real mic room noise deterministically return hallucinated sentences in en, vi and auto. Groq's `no_speech_prob` was 0.97 on the 30 s clip but only 0.15 on the 16 s clip, so filtering on it alone is not enough.

## Fix

Give the cloud branch of `EngineChain` the same speech detection the local path has, with the Silero VAD model (`models::VAD_MODEL`, 885 KB):

- Inject speech detection into `EngineChain` as a seam (for example a boxed detector that returns speech sample ranges), so core does not depend on whisper-rs. src-tauri wires it to `WhisperVadContext` from the whisper crate. Load the VAD model alone, without the Whisper model.
- In the cloud branch of `EngineChain::transcribe`, send only speech: merge VAD speech ranges with a little padding (about 200–300 ms each side) and join gaps shorter than about 2 s. Send each merged region (still split by `cloud_span_bounds` limits) as its own span, and shift returned segment and word times back to their position in the original audio. Never upload a region with no speech. This also stops silence from using the Managed transcription allowance.
- `EngineChain::has_speech` uses the detector when one is available, so background transcription and language detection skip noise-only audio.
- When the VAD model is missing or fails to load, keep the current behaviour and log the existing `VadUnavailable` event. If the model is not installed and a cloud engine is in the chain, download it the way the local path does when that already exists for the local path; otherwise only use it when installed. Say in your report which you did.
- Do not change the gateway repo.

## Acceptance criteria

- [ ] A regression test in `crates/core/tests` builds an `EngineChain` with a fake cloud engine that records the samples it receives and a fake speech detector. Noise-only audio produces no cloud call and no segments.
- [ ] Audio of speech, then 20 s of non-speech, then speech produces two uploads that do not contain the 20 s gap, with segment times placed correctly on the original timeline.
- [ ] Background transcription skips a noise-only span when the detector says it holds no speech.
- [ ] Without a detector, behaviour is unchanged (existing tests pass).
- [ ] `cargo test -p meeting_minutes_core` and `cargo clippy -p meeting_minutes_core -- -D warnings` pass.
