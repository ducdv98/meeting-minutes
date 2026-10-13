# First-run guide

Status: ready-for-agent
Blocked by: 07, 11
Spec: ../spec.md

## What

Guide shown on first launch, skippable and reopenable from Help: Welcome → Recording notice → choose a Whisper model (size versus accuracy, recommended from this PC's CPU/RAM per ADR-0002, download continues in the background with progress) → Microphone and System audio level check (play a test sound, speak) → optional AI setup (detect Ollama, or enter a key) → Library empty state. Completing it counts as seeing the Recording notice.

## Acceptance criteria

- [ ] Shown once; skip and reopen work; state persists.
- [ ] Model download continues after leaving the guide; Record works before it finishes and Processing waits for it.
- [ ] Level check shows live levels for both Sources and warns if either is silent.
- [ ] Help → Open the setup guide reopens it (pass `onOpenGuide` to `HelpView` in `App.tsx`; ticket 11 left the hook).
- [ ] Unit tests for step state and model recommendation.
- [ ] Manual check on the laptop with app data cleared.
