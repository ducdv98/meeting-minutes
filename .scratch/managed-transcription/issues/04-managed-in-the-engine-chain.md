# Managed transcription in the engine chain

Status: ready-for-human
Spec: ../spec.md
Blocked by: 02, 03

## What

- Build the chain as Managed (when connected) → Groq (own key) → Gemini → local Whisper, both for Processing after Stop and Re-transcribe and for the background transcriber during a Recording. Both use the one shared token manager.
- Auto Meeting language: when Managed is first in the chain, detect the language before the first span with the next engine that can (Groq if a key is set, otherwise local), then pass the explicit language to Managed for every span.
- Rename `background::start_decision`'s `has_groq_key` to `has_cloud_engine`. It is true when connected to Managed or a Groq key is set.
- Fallback notice wording for Managed:
  - "Managed transcription is busy" (Quota)
  - "Managed transcription was disconnected" (Unauthorized)
  - "Managed transcription is unavailable" (Offline or Other)
- When the token manager reports Disconnected mid-run, the run carries on with the rest of the chain, and Settings shows the disconnected state (ticket 02).
- Copy diagnostics shows Managed in engines used and audio per engine.

## Acceptance criteria

- [ ] Core tests with fake engines:
  - Managed only;
  - Managed busy midway, so the rest goes to Groq and one Revision results;
  - Managed busy and no Groq key, so the rest goes to Gemini or local;
  - Managed disconnected midway;
  - not connected, so the chain and output are unchanged from today;
  - auto language detected by Groq or local and then sent to Managed.
- [ ] `start_decision` tests: Managed connected without a Groq key starts cloud background transcription on an integrated GPU on battery.
- [ ] The fallback notice text for Managed appears in the Meeting view (UI test).
