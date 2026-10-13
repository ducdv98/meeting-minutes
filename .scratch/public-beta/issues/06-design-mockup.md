# Clickable mockup of the app and website

Status: ready-for-human
Blocked by: none
Spec: ../spec.md

## What

Claude builds a clickable HTML mockup, in light and dark, of: Library (with search and empty state), Meeting view (player, Transcript, Summary, Ask with Citations), recording in progress, first-run guide, Settings, Help/About, and the website Home, Privacy and Support pages. Style and tokens: `docs/design/visual-identity.md`. Website pages follow `website/DESIGN.md`. Uses the brand marks per ADR-0004. Where the mockup shows a token needs to change, update the identity file in the same change, so it stays the single source that ticket 07 implements.

## Acceptance criteria

- [ ] Every screen above exists in light and dark.
- [ ] Every value used comes from (or is added to) `docs/design/visual-identity.md`.
- [ ] App and website read as the same product side by side.
- [ ] The owner approves the mockup; the link and approved version are recorded here.

## Comments

### 2026-10-13: mockup v1 ready for review

Source: `docs/design/mockup.html` (one self-contained page, sample data). Published privately at https://claude.ai/artifact/SEyFm9SNGkAip7FTUhQAhp (version 1).

- App: Library (empty state), Meeting (player with Markers, Transcript with Speaker labels, Summary with Citations, Ask with Answered / Insufficient evidence / Incomplete), Recording (bar, levels, device-switch notice), Library search, Settings, Help/About with Copy diagnostics, first-run guide (5 steps).
- Website: Home (hero with a live scaled copy of the Meeting view, release published vs not-yet toggle, roadmap, FAQ), Privacy, Support.
- Light, dark and system themes. Every value comes from `docs/design/visual-identity.md`; no token changes were needed.
- Waiting on the owner's approval.
