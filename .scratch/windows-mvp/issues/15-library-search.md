# Keyword search across the Library

Status: resolved
Blocked by: 10
Spec: ../spec.md

## What

Full-text search (SQLite FTS5) over all current Transcripts, kept in sync with edits and new Revisions. Results show Meeting, time and snippet, and open the Meeting at that Segment.

## Acceptance criteria

- [x] Searching a word spoken in two Meetings lists both with snippets.
- [x] Clicking a result opens the Meeting and plays the Segment.
- [x] Index rebuild (ticket 04) also rebuilds the search index.

## Comments

### 2026-10-12: implemented; laptop check pending

Design: `../15-core-design.md`. The SQLite index is now schema v3 with an FTS5 `segments` table (`unicode61 remove_diacritics 2`), holding only the current Revision. It is updated on edit, new Revision, rename, delete and rescan, and rebuilt when the index is fresh. The query is built from alphanumeric pieces as prefix terms, so user input can't break FTS syntax. Each Meeting returns at most 5 hits, 100 in total. Results carry their Revision: a hit from an older Revision opens the Meeting without playing a possibly different span. Results refresh on `library-changed`.

Manual check on the laptop: search for a word spoken in two Meetings, click a hit (it should play), and try a Vietnamese word typed without accents.

### 2026-10-13: laptop check passed

The owner ran the manual checks on the laptop and all passed. Resolved.
