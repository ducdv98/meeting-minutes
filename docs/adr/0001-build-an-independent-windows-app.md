# Build an independent Windows meeting recorder

> Superseded by ADR 0020: Meeting Minutes is now a desktop app for Windows and macOS.

We build Meeting Minutes as an independent Windows app. Its core workflow is Record → Transcribe → Ask with Citations. Recordings must survive crashes through durable Chunks, and Citations must point to stable Segment IDs. We design and implement these behaviours in this repository, choosing the architecture for the app's Windows requirements.

## Consequences

- Own and test the recording, Transcript and Citation behaviour in this codebase.
- Choose the UI, capture, Processing and storage technologies for our Windows requirements (see ADR-0003).
