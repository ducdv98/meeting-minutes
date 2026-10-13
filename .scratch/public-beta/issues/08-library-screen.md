# Redesign the Library screen and Library search

Status: ready-for-human
Blocked by: 07
Spec: ../spec.md

## What

Rebuild the Library list, Library search and Import entry point to the approved mockup, including the empty state ("Record or import a file") and delete confirmation.

## Acceptance criteria

- [x] Matches the mockup in light and dark.
- [x] Loading, empty, no-results and error states use the shared components.
- [x] Existing Library, search, import and delete behaviour and tests unchanged.

## Comments

### 2026-10-13

Built the 248px Library shell, grouped Meeting rows, sidebar search with Ctrl K, main-pane results and empty state, token-based import and processing notices, row action menus, and Dialog delete confirmation. Kept the existing Recording, Meeting, Settings, Import, search-at-Segment, and delete flows. Updated tests for the new markup and added shortcut, grouping, language, and delayed Recording-state coverage. Verified with `pnpm typecheck`, `pnpm test` (88 passing), and `pnpm build`. Laptop check: inspect light and dark rendering, drag-and-drop overlay, and the Windows global Record shortcut in the installed app.
