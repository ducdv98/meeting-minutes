# Groq and Gemini keys in Settings

Status: ready-for-human
Spec: ../spec.md

## What

- Add `Groq` and `Gemini` to `ProviderKind` (`crates/core/src/ai/mod.rs`). Both are cloud providers (`is_cloud()`), and their keys go through the existing `SecretStore` and `save_api_key` / `delete_api_key` commands (`src-tauri/src/main.rs`).
- Groq is used only for transcription. It isn't offered as a Summary or Ask provider.
- Settings shows a "Cloud transcription" section with:
  - a Groq key field and a Gemini key field;
  - a "Get a free key" link for each (Groq console API keys, Google AI Studio API keys);
  - a "Test" button that makes one cheap authenticated call and shows OK or the provider's error.
- When the first Groq or Gemini key is saved, show a one-time notice saying Meeting audio or text will be sent to that provider when Processing runs, and that Gemini's free tier may use content for training. Record that it was shown in Settings, so it's never shown again.

## Acceptance criteria

- [ ] Keys are saved, read and deleted through `SecretStore`. A memory-store test covers Groq and Gemini.
- [ ] Keys never appear in logs, diagnostics or Settings JSON.
- [ ] Settings UI tests: the links open the provider pages, Test shows success and failure, and the one-time notice shows once.
- [ ] Settings that existed before this change load unchanged.
