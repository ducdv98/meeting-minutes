# AI provider setup: Ollama and BYOK

Status: ready-for-human
Blocked by: 01
Spec: ../spec.md

## What

Settings for text-generation providers used by Summary and Ask: detect Ollama at localhost:11434 and list its models; or enter an OpenAI / Anthropic API key, stored in Windows Credential Manager. One provider-neutral interface with streaming and structured (JSON schema) output. A cloud send shows a disclosure of exactly what is sent, with consent recorded once per Meeting.

## Acceptance criteria

- [ ] With Ollama running, its models are listed and one can be selected; a test prompt succeeds.
- [x] API keys never appear in files, logs or the index.
- [ ] No provider configured → Summary/Ask show setup guidance; recording and transcription still work.
- [x] Unit tests with a fake provider for streaming, schema output and errors.

## Comments

### 2026-10-12: implemented; laptop check pending

Code is done and tested on Linux. The design is in `../12-core-design.md`. Codex ran out of quota partway through, so Claude finished the work and ran the reviews.

- Core (`crates/core/src/ai/`):
  - `TextProvider` is one provider-neutral interface with streaming (`on_delta`), cancel, and `generate_json` for schema output.
  - Clients for Ollama (`/api/chat` NDJSON with `format`), OpenAI (Chat Completions SSE with strict `json_schema`) and Anthropic (Messages SSE with `output_config.format`, default model `claude-opus-5-5`). Each sits behind an `HttpClient` seam, so the wire formats are unit tested without a network.
  - Errors are typed: auth, refused, truncated, invalid output, unreachable, key store.
  - API keys:
    - An `ApiKey` cannot be serialized and its Debug output is redacted.
    - A 401/403 response body is discarded, and other error text has the key replaced with `***`.
    - Keys are never a settings field.
- Settings: `settings.json` gains `ai` (selected provider and a model per provider).
- Consent: `meeting.json` gains `cloud_consent` (per Meeting and per cloud provider, idempotent), with `Library::cloud_consent` and `Library::record_cloud_consent`.
- App:
  - Keys are stored in Windows Credential Manager (keyring 3.6.3).
  - A Settings "AI provider" section lets you pick a provider, find Ollama models or load cloud models, save or remove a key, and send a test message.
  - The Meeting view shows Summary/Ask setup guidance when no provider is configured.
  - `CloudDisclosure` (shows exactly what will be sent) is built and tested, but no flow uses it yet. Tickets 13 and 14 wire it in with the consent record.
- Verified here: core tests, clippy and Vitest pass. `main.rs` passes Windows-gnu clippy against a stub whisper crate.
- Manual check on the laptop:
  1. With Ollama running (`ollama pull llama3.2`), go to Settings → AI provider → Ollama → Find Ollama models, choose one, then Test. You should get a reply.
  2. Stop Ollama and click Find Ollama models again. You should see the "not running" guidance.
  3. Choose OpenAI or Anthropic, paste a key, Save key, Load models, then Send test message. Check that the key is listed in Credential Manager under "Meeting Minutes", and is not in `settings.json` in app data or any Meeting folder.
  4. With Provider = None, open a Meeting. You should see the "Summary and Ask" setup guidance, and recording and transcription should still work.
