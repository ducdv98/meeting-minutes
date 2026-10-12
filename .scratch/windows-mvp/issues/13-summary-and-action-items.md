# Summary and Action items with Citations

Status: ready-for-human
Blocked by: 09, 12
Spec: ../spec.md

## What

Generate the Summary (Overview, Key points, Decisions, Action items with owner/due date when stated) from the current Transcript Revision. Every item carries Segment-ID Citations validated by app code; items without a valid Citation are dropped. Runs automatically after local transcription only when the provider is local; cloud requires a click. Writes `summary.json` + `summary.md`; Citations in the UI play their span.

## Acceptance criteria

- [ ] Summary appears after Processing on the laptop with Ollama.
- [ ] Every visible item has ≥1 Citation that resolves in this Meeting and Revision; invented IDs are dropped (unit test).
- [ ] Long transcripts that exceed the model context are handled (chunked map-reduce or retrieval), not silently truncated.

## Comments

### 2026-10-12: implemented; laptop check pending

Code is done and tested on Linux. The design is in `../13-core-design.md`.

- Core: `citation` (validates model-written Segment IDs against this Meeting's Transcript Revision and the IDs actually sent; ms ranges are copied from the Transcript) and `summary` (`summarize` with map-reduce chunking when the Transcript exceeds the model context; nothing is truncated; items with no valid Citation are dropped and counted). Writes `summary.json` and `summary.md`. Ollama requests now set `num_ctx` 8192.
- App: the Summary panel shows Overview, Key points, Decisions and Action items (owner/due when stated), each Citation plays its span, and it notes when the Summary is from an older Revision. After a transcription the Summary runs on its own only when the provider is Ollama. Cloud providers need a click, with the disclosure shown and consent recorded the first time.
- Decision: once consent is recorded for a Meeting and provider, later cloud sends need the click but don't show the disclosure again (as designed in ticket 12).
- Manual check on the laptop: with Ollama set up, record or import a Meeting. The Summary should appear after transcription, and each Citation should play its span. With OpenAI/Anthropic nothing is sent until you click "Generate Summary with …" and then Send.
