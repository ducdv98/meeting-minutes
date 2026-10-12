# Keyword search across the Library

Status: ready-for-agent
Blocked by: 10
Spec: ../spec.md

## What

Full-text search (SQLite FTS5) over all current Transcripts, kept in sync with edits and new Revisions. Results show Meeting, time and snippet, and open the Meeting at that Segment.

## Acceptance criteria

- [ ] Searching a word spoken in two Meetings lists both with snippets.
- [ ] Clicking a result opens the Meeting and plays the Segment.
- [ ] Index rebuild (ticket 04) also rebuilds the search index.

## Comments
