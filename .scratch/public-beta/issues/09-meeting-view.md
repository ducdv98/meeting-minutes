# Redesign the Meeting view

Status: ready-for-human
Blocked by: 07
Spec: ../spec.md

## What

Rebuild the Meeting view to the approved mockup: Timeline player with Markers, Transcript with Speaker labels and Segment editing, Summary with Action items, Ask panel with Answer states (Insufficient evidence, Incomplete, Unverified), Citations, cloud disclosure and Processing progress.

## Acceptance criteria

- [ ] Matches the mockup in light and dark.
- [x] Every Answer state and Processing state has a distinct, readable design.
- [x] Citations look the same everywhere and still play their span.
- [x] Existing behaviour and tests unchanged.

## Comments

- 2026-10-13: Rebuilt the Meeting header, Timeline player, Transcript, Summary and Ask rail to the approved mockup using the shared tokens and UI components. Added recorded Source metadata and an Open folder command, Citation span highlighting, cloud disclosure Dialog, and shared loading, empty and error states. Updated markup-sensitive tests and added coverage for Answer states, Sources, folder access and Citation playback. Verified `pnpm typecheck`, `pnpm test` (91 passed), `pnpm build`, and the Windows check (`Finished` with no warnings or errors). Laptop check remains: compare spacing and both themes against the mockup in the Windows app, and exercise real audio playback, folder opening, and cloud disclosure.

### 2026-10-13: review fixes finished by Claude

Codex hit its usage limit during the review-fix pass. Claude checked each of the 13 review findings against the code: Meeting switching resets all Meeting state (keyed view plus a synchronous reset), reloads are versioned per data stream, Transcript and the Summary/Ask rail show Loading until their data arrives, Ask shows the backend's fixed Insufficient-evidence and Off-topic text with a distinct Off-topic label, and older-Revision Citations have hover text. Claude added the missing race tests in `src/MeetingView.test.tsx` (switching with late old data, overlapping reloads resolved in reverse order, loading vs empty) and confirmed the first two fail when the version checks are removed.
