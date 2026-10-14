# Laptop check with a real invite

Status: ready-for-human
Spec: ../spec.md
Blocked by: 04

## What

On the Windows laptop, with a real Invite code from the operator, check the end-to-end flow from the handoff doc.

## Acceptance criteria

- [ ] Connect with the invite. Settings shows Connected, and the Credential Manager holds the entries.
- [ ] Record a meeting of 5 minutes or more with both Sources. Spans upload during the Recording, and after Stop the Transcript shows Managed transcription as the engine.
- [ ] An auto-language Meeting (Vietnamese) gets the right language.
- [ ] Leave the app idle for more than 15 minutes, then record again. The token refreshes without a prompt.
- [ ] The operator revokes the client mid-Recording. The Meeting finishes on Groq, Gemini or local, the notice says "disconnected", and Settings shows the disconnected message.
- [ ] Copy diagnostics shows Managed, error codes and request IDs, and no tokens.
