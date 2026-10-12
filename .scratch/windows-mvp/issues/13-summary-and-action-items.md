# Summary and Action items with Citations

Status: ready-for-agent
Blocked by: 09, 12
Spec: ../spec.md

## What

Generate the Summary (Overview, Key points, Decisions, Action items with owner/due date when stated) from the current Transcript Revision. Every item carries Segment-ID Citations validated by app code; items without a valid Citation are dropped. Runs automatically after local transcription only when the provider is local; cloud requires a click. Writes `summary.json` + `summary.md`; Citations in the UI play their span.

## Acceptance criteria

- [ ] Summary appears after Processing on the laptop with Ollama.
- [ ] Every visible item has ≥1 Citation that resolves in this Meeting and Revision; invented IDs are dropped (unit test).
- [ ] Long transcripts that exceed the model context are handled (chunked map-reduce or retrieval), not silently truncated.

## Comments
