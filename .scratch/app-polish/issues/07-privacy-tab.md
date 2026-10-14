# Privacy tab

Status: ready-for-agent
Spec: ../spec.md
Blocked by: 03, 05, 06

## What

- One row per place data can leave the PC: Managed transcription, Groq, Gemini, OpenAI, Anthropic, and the Correction model.
- Each row: what is sent (audio / Transcript text), when (automatically after Stop, after transcription, or only when you click), and whether it's on now (connected, key saved, chosen as default).
- An opening line: "Your Library stays on this PC. Nothing is sent unless one of these is on."
- Each row's Details uses the same text as the disclosure next to its control (ticket 06).
- After the cloud notice is acknowledged, its text stays here as a permanent line.

## Acceptance criteria

- [ ] Every way data can leave the PC is listed, and the "on" state matches Settings live.
- [ ] Turning everything off shows every row as off.
- [ ] No privacy text is duplicated in source.
