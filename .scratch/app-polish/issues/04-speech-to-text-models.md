# Speech-to-text models and Transcription order summary

Status: ready-for-agent
Spec: ../spec.md
Blocked by: 02, 03

## What

- Settings stores a list of **Speech-to-text models** (Provider + model ID), and per Provider which one is "Used automatically".
  - Built in: every local Whisper model; Managed transcription when connected (fixed model, can't be removed).
  - User-added: Groq (`whisper-large-v3`, `whisper-large-v3-turbo`) and Gemini models, via Add → Provider → model → Test.
  - The local Whisper "Used automatically" choice replaces today's Transcription model setting.
- Migration: existing settings keep working. A saved Groq key adds `whisper-large-v3` as Groq's automatic model; a saved Gemini key adds the current Gemini model; the current Whisper model becomes the local automatic one.
- Processing uses each Provider's automatic model in the existing **Transcription order** (Managed → Groq → Gemini → local Whisper). The order itself does not change.
- The tab opens with a read-only Transcription order summary: each step, its automatic model and status (Ready, Not connected, No key, Downloads when first used), and "New Meetings use: X".
- Vocabulary stays on this tab.

## Acceptance criteria

- [ ] Adding Groq `whisper-large-v3-turbo` and marking it automatic makes the next Groq transcription use it (Rust test on the request).
- [ ] The summary reflects connect/disconnect, key save/remove and model download state without reopening Settings.
- [ ] Old settings files load with the same behaviour as before (Rust migration test).
- [ ] The Transcript records the engine as today.
