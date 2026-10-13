# Clickable mockup of the app and website

Status: ready-for-agent
Blocked by: none
Spec: ../spec.md

## What

Claude builds a clickable HTML mockup, in light and dark, of: Library (with search and empty state), Meeting view (player, Transcript, Summary, Ask with Citations), recording in progress, first-run guide, Settings, Help/About, and the website Home, Privacy and Support pages. Style and tokens: `docs/design/visual-identity.md`. Website pages follow `website/DESIGN.md`. Uses the brand marks per ADR-0004. Where the mockup shows a token needs to change, update the identity file in the same change, so it stays the single source that ticket 07 implements.

## Acceptance criteria

- [ ] Every screen above exists in light and dark.
- [ ] Every value used comes from (or is added to) `docs/design/visual-identity.md`.
- [ ] App and website read as the same product side by side.
- [ ] The owner approves the mockup; the link and approved version are recorded here.
