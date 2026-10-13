# UI foundation: Tailwind, components, themes and window state

Status: ready-for-agent
Blocked by: 06
Spec: ../spec.md

## What

Add Tailwind and Radix-based (shadcn/ui-style) components with the tokens from `docs/design/visual-identity.md` as CSS variables and the Tailwind theme. Bundle Be Vietnam Pro and Inter (no network font loading). Light/dark theme follows Windows. Shared building blocks for loading, empty and error states. The window remembers its size and position, with a larger default. Derive any extra brand variants (white wordmark for dark mode) via `scripts/build-brand-assets.py` (ADR-0004).

## Acceptance criteria

- [ ] Tokens match the approved mockup; theme switches with Windows without restart.
- [ ] Shared Loading/Empty/Error components with tests.
- [ ] Window size and position persist across restarts and stay on-screen if a monitor is removed.
- [ ] Existing tests, typecheck and build pass.
