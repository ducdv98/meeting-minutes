# Replace Medium with Large v3 turbo q5_0

Status: ready-for-human
Blocked by:
Spec: ../spec.md

## What

Large v3 turbo is faster than Medium (4 decoder layers against 24) and more accurate (ADR 0008).

- Change the model list to Base, Small, and Large v3 turbo `ggml-large-v3-turbo-q5_0.bin` (547 MiB), with its size and SHA-256 taken from the Hugging Face `ggerganov/whisper.cpp` files.
- Drop Medium and the unquantized turbo file.
- Migrate any `settings.json` that names `medium` or `large-v3-turbo` to the new turbo entry.
- Downloaded files that are no longer used may be deleted.
- Update the Settings descriptions. The memory note "Needs 16 GB" no longer applies.
- The first-run model recommendation (ticket 12): recommend turbo when a GPU is available (after ticket 19), otherwise Small.

## Acceptance criteria

- [ ] Settings lists Base, Small and Large v3 turbo, with correct sizes. Downloads verify SHA-256.
- [ ] A settings file with `medium` loads as the new turbo model; a core test covers this.
- [ ] Vietnamese and English test Meetings transcribe with the new turbo model on the laptop. The owner checks Vietnamese quality is no worse than Medium's.

## Comments

### 2026-10-13: implemented; laptop check pending

Codex implemented this ticket and Claude reviewed it.
- The model list is now Base, Small and `large-v3-turbo-q5_0`: 574,041,195 bytes, SHA-256 `3942…a7e2`, checked against the Hugging Face LFS pointer.
- A `settings.json` set to `medium` or `large-v3-turbo` loads as `large-v3-turbo-q5_0`. Old model files are left on disk.
- The first-run recommendation (turbo when a GPU is present) waits for ticket 12 and ticket 19.
- Laptop check (the owner): transcribe one English and one Vietnamese Meeting with Large v3 turbo, and compare its Vietnamese with Medium's.
