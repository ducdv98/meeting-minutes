# Language models, Use for defaults and Correction consent

Status: ready-for-agent
Spec: ../spec.md
Blocked by: 02, 03

## What

- Settings stores a list of **Language models** (Provider + model ID) for Ollama, OpenAI, Anthropic and Gemini, added via Add → Provider → model from the Provider's list (existing `listAiModels`) or a typed ID → Test.
- **Use for** defaults: Correction (a Language model, or Off), Summary, Ask. Summary and Ask can use different Providers.
- Choosing a cloud model for Correction shows a one-time confirmation: "Transcripts will be sent to X after every transcription." Cancel keeps the old choice. Ollama and Off need none. Changing to a different cloud Provider asks again.
- Correction runs on the chosen model, or is skipped when Off.
- Migration: today's provider + model becomes one Language model used for both Summary and Ask. A saved Gemini key makes Gemini's current model the Correction default, already confirmed (it was the existing behaviour). Without a Gemini key, Correction is Off.
- Each model row has Test (existing test message) and Remove. Removing a model used by a default asks which model replaces it, or sets that default to none.

## Acceptance criteria

- [ ] Summary on Anthropic and Ask on Ollama work at the same time (tests on the request each sends).
- [ ] Correction uses the chosen Provider, and doesn't run when Off (Rust tests).
- [ ] The confirmation appears for a cloud Correction model and not for Ollama or Off.
- [ ] Old settings files load with the same behaviour as before (Rust migration test).
