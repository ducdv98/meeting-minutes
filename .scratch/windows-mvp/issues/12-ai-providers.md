# AI provider setup: Ollama and BYOK

Status: ready-for-agent
Blocked by: 01
Spec: ../spec.md

## What

Settings for text-generation providers used by Summary and Ask: detect Ollama at localhost:11434 and list its models; or enter an OpenAI / Anthropic API key, stored in Windows Credential Manager. One provider-neutral interface with streaming and structured (JSON schema) output. A cloud send shows a disclosure of exactly what is sent, with consent recorded once per Meeting.

## Acceptance criteria

- [ ] With Ollama running, its models are listed and one can be selected; a test prompt succeeds.
- [ ] API keys never appear in files, logs or the index.
- [ ] No provider configured → Summary/Ask show setup guidance; recording and transcription still work.
- [ ] Unit tests with a fake provider for streaming, schema output and errors.

## Comments
