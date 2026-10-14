# Motion coverage

Status: ready-for-agent
Spec: ../spec.md

## What

Apply the motion table in `docs/design/visual-identity.md` → Motion everywhere it's missing:
- Dialogs (`components/ui/Dialog`, ConfirmDelete, CloudDisclosure): fade + scale 0.98 → 1, 240ms.
- Dropdowns, tooltips, popovers: fade + 4px slide from the trigger side, 180ms (Radix `data-state` / `data-side` attributes).
- Main view change in `App.tsx` (Library, Meeting, Search, Settings, Help): opacity cross-fade, 180ms.
- Loading skeleton → content: opacity fade, 180ms.
- "Saved" indicator in Settings: fade in, fade out after about 2s.
- A Meeting newly added to the sidebar list: fade + 8px rise once. Not on first load.
- Draft → Revision in the Meeting view: Transcript cross-fade, keeping the scroll on the Segment nearest the old top row.

Use the duration tokens and `ease-functional` only. No animation library. Settings tab and Details disclosure motion come with tickets 02 and 06.

## Acceptance criteria

- [ ] Every row of the motion table except the two above is implemented with the listed duration.
- [ ] With `prefers-reduced-motion`, nothing moves (the global rule in `tokens.css` still applies; no inline durations bypass it).
- [ ] Nothing loops or starts on its own except progress, live levels and the recording pulse.
- [ ] Existing UI tests pass; animations don't delay content becoming queryable in tests.
