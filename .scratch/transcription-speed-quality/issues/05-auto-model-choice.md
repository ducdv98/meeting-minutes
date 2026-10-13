# Auto model choice

Status: needs-info
Spec: ../spec.md
Blocked by: 04

## What

- Add an **Auto** entry to the Settings model list and make it the default for new installs.
- Auto maps the device chosen by ticket 01 (discrete GPU with its memory, integrated GPU, or CPU) to a model, using a table taken from ticket 04's results. A starting point until then: discrete GPU → turbo q8_0 or better; integrated GPU → `large-v3-turbo-q5_0`; CPU → `small`.
- Add any models the table needs to `MODELS` (with size and sha256) in `crates/core/src/models.rs`.
- Settings that already name a model keep it. Settings with no model, or the old default `small` that was never explicitly chosen, move to Auto.
- The resolved model is recorded in the Transcript (`model`) and in diagnostics. Auto is never written there.
- The model is downloaded on first use, as today.

## Acceptance criteria

- [ ] Unit tests for the device → model table, including "won't fit in VRAM → next lighter model".
- [ ] Settings migration tests: explicit choices are kept, and defaults move to Auto.
- [ ] Settings UI shows "Auto (uses <model> on <device>)".

Needs info: the table from ticket 04.
