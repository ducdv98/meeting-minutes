# Record the cloud Speech-to-text model on a Revision

Status: needs-triage
Spec: ../spec.md

## What

Ticket 12 records which Provider made a Revision (Managed transcription, Groq, Gemini, Local + Whisper model), but not the exact cloud model ID (e.g. `whisper-large-v3-turbo`). Now that a user can pick Groq or Gemini models, the Revision badge and diagnostics should show it.

- `EngineName::Groq` and `EngineName::Gemini` (crates/core/src/transcript.rs) carry the model ID, like `Local(String)` does.
- Old Transcript files without a model still load (the label stays "Groq" / "Gemini").
- Diagnostics engine parsing keeps working.

## Acceptance criteria

- [ ] A Revision made with Groq `whisper-large-v3-turbo` shows "Groq · whisper-large-v3-turbo".
- [ ] Old Transcripts load unchanged (Rust test).
