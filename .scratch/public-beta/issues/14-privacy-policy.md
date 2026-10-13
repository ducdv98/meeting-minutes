# Privacy policy

Status: ready-for-agent
Blocked by: 02, 13
Spec: ../spec.md

## What

Write the privacy policy from the code, not from intentions: list every network destination the app can contact (Hugging Face model downloads, GitHub update check, Ollama on localhost, OpenAI/Anthropic on explicit click), what each receives, what is stored where (Library, app data, Credential Manager, log), no telemetry, and the recording consent responsibility. Publish it at `/privacy` and link it from the app's Help.

## Acceptance criteria

- [ ] Every outbound call in the code is listed; a reviewer can trace each claim to code.
- [ ] Dated; contact email included.
