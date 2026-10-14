# Page header and notification area

Status: ready-for-agent
Spec: ../spec.md

## What

- A shared `PageHeader` component: title, one line of context, actions on the right. Use it on Library home, Meeting, Search, Settings and Help.
  - Meeting: Title, date · duration · language, actions (⋯ menu, Summary/Ask toggles as they exist).
- A single notification area under the header for import progress, import errors, unreadable folders and delete errors (today stacked banners in `App.tsx`). Each item is dismissible where it is today; progress items are not.
- Keep the recording bar where it is.

## Acceptance criteria

- [ ] Every view starts with the same header layout and spacing.
- [ ] No banner renders outside the notification area.
- [ ] Existing `App.test.tsx` flows (import, delete error, malformed folders) pass with updated queries.
