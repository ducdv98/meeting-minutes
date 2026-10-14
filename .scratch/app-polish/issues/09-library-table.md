# Library table home

Status: ready-for-agent
Spec: ../spec.md
Blocked by: 08

## What

- With no Meeting selected, the main pane shows a Library table: Title · Date · Duration · Language · Status.
  - Status: processing stage and %, Failed, Summary made, or nothing.
- Sort by Title, Date (default, newest first) and Duration; clicking a header toggles direction.
- Clicking or pressing Enter on a row opens the Meeting. The ⋯ menu per row matches the sidebar (Rename, Delete, Cancel processing).
- Empty Library keeps today's empty state. The sidebar list stays.

## Acceptance criteria

- [ ] Table rows match the sidebar Meetings and update on Library changes and processing events.
- [ ] Sorting is keyboard-accessible and announced (`aria-sort`).
- [ ] UI tests: sort by each column, open a Meeting from a row.
