# Tidy the engine chain after the cloud transcription review

Status: resolved
Spec: ../spec.md
Blocked by: 03, 04, 05, 06, 07

## What

This is a refactor and must not change behaviour. The standards review of `b6b170e..b2cff12` raised these smells. Each one is a judgement call, not a bug. Every existing test must pass unchanged, except tests that only mention a renamed item.

1. **`SpeechEngine` carries chain-only methods.** `crates/core/src/processing.rs:28` gained `engines_used`, `fallback_reason`, `engine_audio_ms`, `is_offline`, `is_deferred` and `detect_language_with_cancel`. Only `EngineChain` implements them, and the other engines inherit do-nothing defaults. Move them to `EngineChain`, or to one `chain_report()` that returns a small struct, so callers that need them take the chain itself.
2. **Engine names are plain strings.** `engine_audio_ms` returns a `(groq, gemini, local)` tuple, and `Transcript.model: Vec<String>` (`transcript.rs:45`) now holds engine names. Add an `EngineName` enum (Groq, Gemini, Local model) with its display label. Use it for usage totals, keyed by engine, and for the engine list on the Transcript. The saved JSON must stay readable both ways: old Transcript files load, and the field name and value strings on disk stay as they are, or are covered by a serde alias.
3. **Fallback reasons are strings parsed by suffix.** `diagnostics.rs:470` matches `ends_with("daily limit reached")` and similar phrases. Make the reason a `FallbackReason` enum (quota, unauthorized, offline, rate limit, other) with a `Display` for the Meeting view text. Diagnostics then match the variant.
4. **The two notices live in different places.** The fallback notice is on the Transcript (`fallback_reason`) and the Correction notice is in meeting metadata (`correction_notice`). Pick one home, and record why in a short comment or ADR 0011 note. Keep existing Meeting files loading.
5. **The "not supported for Summary or Ask yet" error is written four times.** It appears in `ai/mod.rs:282` and `:307`, and in `ai/http.rs:549` and `:686`. The `Groq` arm at `ai/mod.rs:307` can never run, because the early return at `:280` already handled Groq. Replace all four with one helper, such as `AiError::unsupported_for_text(kind)` or a `ProviderKind::supports_text()` check made once, and delete the dead arm.
6. **The timestamp offset loop is written four times.** The same "add an offset to each segment's and word's start and end" loop is in `cloud/groq.rs:75` and `:113`, `cloud/gemini.rs:141` and `processing.rs:453` and `:728`. Extract one helper, such as `Segment::shift(ms)` or `shift_segments(&mut [Segment], ms)`.
7. **The default Gemini model is written in several places.** `"gemini-2.5-flash"` appears in `correction.rs:201`, `settings.rs:59`, `ai/mod.rs:329` and `src-tauri/src/main.rs:1283`. Make it one constant. Remove any `unwrap_or` fallback that can no longer be reached, because `model_for(Gemini)` already applies the default.
8. **Code used only by tests.**
   - `background::start_decision`'s `online` argument is always `true` at both call sites (`main.rs:1297`, `:1373`). Drop the argument, or pass real network state.
   - `GroqEngine`'s `SpeechEngine` impl (`cloud/groq.rs:257`) and `groq::with_default_http` (`:395`) are used only by tests. Move them under `#[cfg(test)]`, or into the test helpers.
9. **Errors are thrown away.** `groq.rs` turns parse and transport errors into `EngineError::Other` and drops the cause, and Correction's `Err(_)` arms in `main.rs` (for example around `:1586`) drop the error too. Keep the error kind, or a short cause, so diagnostics can count it. Never keep a key, audio, or Transcript or Meeting text in an error, a log or diagnostics.

Small separate commits are fine. Items 5–7 are independent of 1–4.

## Acceptance criteria

- [x] No user-visible change. Meeting view notices, diagnostics fields and on-disk JSON are the same. A test loads a Transcript and Meeting file saved before this ticket.
- [x] `SpeechEngine` has only the methods every engine uses.
- [x] Engine names and fallback reasons are enums, with no suffix matching on strings.
- [x] The Summary/Ask unsupported error, the offset shift and the default Gemini model are each defined once, and the dead Groq arm is gone.
- [x] `start_decision` has no argument that is always the same value. Code used only by tests is under `#[cfg(test)]`.
- [x] Engine errors keep their kind for diagnostics. No key or Meeting audio or text appears in errors, logs or diagnostics.
- [x] Checks pass: core tests, clippy, the frontend tests, `tsc`, and the Windows clippy check on `src-tauri/src/main.rs`.

## Comments

- Done by Codex with two review rounds (standards and spec). Items 1–3 and 5–9 are done. Item 4 is left as-is: moving either notice would change saved Meeting or Transcript JSON, so `meeting_file.rs` has a comment saying why the Correction notice stays in Meeting metadata.
- `chain_report()` now lives only on `EngineChain`. Tests and the Whisper benchmark wrap their local engine with `EngineChain::borrow_local`.
- Checks: 257 core tests, core and whisper clippy, 133 frontend tests, tsc, and Windows clippy on `src-tauri/src/main.rs` (stub whisper crate) all pass.
