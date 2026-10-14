# Smooth progress while cloud engines transcribe

Status: needs-triage
Spec: ../spec.md

## What

With Groq or Gemini in the engine chain, progress is reported only when a whole cloud span (up to 10 minutes) comes back, and each Source is transcribed in turn. A two-Source Meeting under 10 minutes therefore shows 0%, then 50%, then 100% (`EngineChain::transcribe` in `crates/core/src/processing.rs`). The language check, which uploads the opening 30 s, also runs at 0%.

Found while diagnosing the "stays at 0, then jumps to 50%" report. The user's runs were local, and the local path was fixed separately (silence detection now fills the first part of the bar, and each Source is weighted by its speech). This cloud case was reproduced with a fake Groq engine and left for later.

## Acceptance criteria

- [ ] While a cloud span is uploading or transcribing, the bar keeps moving: an estimate from the span's audio length that stops short of the span's end.
- [ ] No extra requests to Groq: the free tier allows about 20 requests a minute.
- [ ] A test drives a two-Source Meeting through a cloud `EngineChain` and asserts that no single progress step exceeds a set size.
