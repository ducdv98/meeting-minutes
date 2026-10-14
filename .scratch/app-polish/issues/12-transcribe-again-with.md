# Transcribe again with a chosen model

Status: ready-for-agent
Spec: ../spec.md
Blocked by: 04

## What

- In the Meeting's ⋯ menu (and the Transcribe button when "Process after I stop" is off): "Transcribe…" / "Transcribe again with…" opens a dialog with the shared picker (ticket 11's component if done, else the same grouping) listing Speech-to-text models with status, plus "Automatic (Transcription order)" as default.
- A chosen model transcribes this run only, with no fallback: if it fails, the run fails with that model's error and the previous Revision stays current.
- Automatic follows the Transcription order as today. The automatic run after Stop always uses the order.
- The result is a new Revision recording the engine and model. Correction then runs with its default.
- Cloud choices show the existing consent behaviour for that Provider.

## Acceptance criteria

- [ ] Choosing Local large-v3 for a Meeting with a Groq key saved transcribes locally (test that no Groq request is made).
- [ ] A chosen cloud model that fails does not fall through and leaves the old Revision current.
- [ ] Automatic behaves exactly as before.
- [ ] Re-transcribing during a Recording of the same Meeting stays disabled as today.
