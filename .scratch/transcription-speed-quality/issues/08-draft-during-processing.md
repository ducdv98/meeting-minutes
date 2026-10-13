# Show a Draft during Processing

Status: ready-for-human
Spec: ../spec.md
Blocked by: 07

## What

- While a Meeting is transcribing, emit newly finished Segments to the frontend as they are produced, per Source and in time order. Use a Tauri event carrying the Meeting ID and the Segments, with no IDs.
- The Meeting view shows them as a **Draft**:
  - read-only, with a clear "Transcribing…" state;
  - no Citations, no Ask or Summary actions, no Participant moves, no Segment editing;
  - shown with Participant names, as the Transcript is.
- Library search never indexes a Draft.
- When Processing completes, the Draft is replaced by the published Revision. When it fails or is cancelled, the Draft is cleared and the previous Revision (if any) is shown, as today.
- A Draft is never written to the Library.

## Acceptance criteria

- [ ] Frontend tests: Draft Segments render as they arrive, Ask and Citations are disabled, and the Draft is swapped for the Revision on completion and cleared on failure.
- [ ] Core test: the progress callback delivers Segments in order and never reports more audio than the total.
- [ ] Re-transcribing a Meeting that already has a Revision shows the Draft in place of the old Revision, and restores the old one on cancel.
- Laptop check: Re-transcribe a Meeting: the Draft replaces the old Revision with "Draft · Transcribing…", Ask and Summary are hidden, closing and reopening the Meeting shows the Draft so far, and Cancel restores the old Revision.
