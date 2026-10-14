# Managed collapse and Details disclosures

Status: ready-for-agent
Spec: ../spec.md
Blocked by: 02

## What

- On the Speech-to-text tab, Managed transcription shows as a "Have an invite code?" link when not connected. Clicking it expands the Invite code field. When connected (or just revoked), the section is expanded.
- Long privacy text next to a control becomes one line plus a "Details" disclosure that expands in place (height + opacity, 180ms):
  - Managed transcription: "Sends Meeting audio to the Meeting Minutes service; nothing is stored." + Details with the current full statement.
  - The cloud notice and Provider notes where they run past one line.
- Put the full texts in one module (e.g. `src/privacy-text.ts`) so ticket 07 reuses them.

## Acceptance criteria

- [ ] A user without an Invite code sees only the link; connecting still works after expanding.
- [ ] The full privacy statement is reachable before Connect (consent stays informed).
- [ ] Disclosures work by keyboard and announce expanded state.
