# UI foundation: Tailwind, components, themes and window state

Status: ready-for-human
Blocked by: 06
Spec: ../spec.md

## What

Add Tailwind and Radix-based (shadcn/ui-style) components with the tokens from `docs/design/visual-identity.md` as CSS variables and the Tailwind theme. Bundle Be Vietnam Pro and Inter (no network font loading). Light/dark theme follows Windows. Shared building blocks for loading, empty and error states. The window remembers its size and position, with a larger default. Derive any extra brand variants (white wordmark for dark mode) via `scripts/build-brand-assets.py` (ADR-0004).

## Acceptance criteria

- [x] Tokens match the approved mockup; theme switches with Windows without restart.
- [x] Shared Loading/Empty/Error components with tests.
- [x] Window size and position persist across restarts and stay on-screen if a monitor is removed.
- [x] Existing tests, typecheck and build pass.

## Comments

- 2026-10-13: Added Tailwind v4 tokens, offline Inter and Be Vietnam Pro subsets, shared Radix controls and states, theme-aware derived brand assets, and window-state persistence. Verified the plugin checks saved coordinates against available monitors before restoring position. `pnpm typecheck`, `pnpm test` (80 tests), `pnpm build`, `cargo test -p meeting_minutes_core`, and Windows-target clippy via `wincheck.sh` pass. Build JS/CSS: 255.86/2.98 kB before; 284.18/34.26 kB after. No commit.
- 2026-10-13: Fixed review findings: distinct source names for colliding Tailwind tokens; a single WOFF2-only face set; a mounted Tooltip provider; React 19 button props; radio descriptions; shared Citation rendering; static skeletons; and a post-restore window clamp with core rectangle tests. Replaced UI class checks with behavior tests. Verified `pnpm typecheck`, `pnpm test` (84 tests), `pnpm build`, `cargo test -p meeting_minutes_core`, and Windows-target `wincheck.sh` (`Finished`, no warnings or errors). Built JS/CSS: 296.30/29.74 kB; 15 WOFF2 assets total 291,144 bytes, no WOFF assets. Built CSS has no self-referencing declarations; all 17 documented light/dark color pairs and radius/text pixel values match. No commit.
