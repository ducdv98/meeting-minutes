# 20: Managed transcription detects the Meeting language

**What to build:** A Meeting on Auto-detect that uses Managed transcription with no local model detects its language from the gateway instead of falling back to English. The gateway already accepts `language=auto` and Groq's `verbose_json` reply carries the detected language, but the gateway's upload reply returns only `request_id` and `segments`. Return the detected language from the gateway, and have `ManagedEngine` detect from the opening the way Groq and Gemini do, so it no longer needs a local model to detect. See `.scratch/v1-launch/spec.md`.

**Blocked by:** a gateway change to return the detected language (`meeting-minutes-transcription-gateway`, `src/routes/transcriptions.ts`)

Status: needs-triage

- [ ] The gateway reply includes the detected language when `language=auto`
- [ ] `ManagedEngine` detects the language, and `needs_detected_language` no longer forces local detection
- [ ] A Vietnamese Meeting on Auto-detect with only Managed transcription comes back in Vietnamese

## Comments

Found in the v1-launch code review. Until this lands, that path falls back to English rather than waiting for a model download nobody started.
