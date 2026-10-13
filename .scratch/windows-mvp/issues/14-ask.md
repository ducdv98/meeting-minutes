# Ask a Meeting with cited Answers

Status: resolved
Blocked by: 10, 12
Spec: ../spec.md

## What

Ask panel on the Meeting view following the spec's Ask rules: scoped to the current Revision, retrieval of relevant Segments (with neighbours) when the Transcript exceeds the model context, structured Answer `{answer, citations[], insufficient_evidence}`, citation validation, streaming, Stop, Incomplete/Unverified states, one active Ask per Meeting, retry for the latest failed question, history in `asks.json`, nudge for off-topic input.

## Acceptance criteria

- [x] Question with support → Answer with clickable Citations that play the right span.
- [x] Question without support → Insufficient evidence, no invented content.
- [x] Stop mid-stream → partial text labelled Incomplete and excluded from later context.
- [x] History survives restart; a failed question can be retried and is not duplicated.
- [x] Unit tests: citation validation (unknown, cross-Meeting, other-Revision IDs rejected), context building, state transitions.

## Comments

### 2026-10-12: implemented; laptop check pending

Code is done and tested on Linux. The design is in `../14-core-design.md`.

- Core `ask`: `asks.json` history, one Pending Ask at a time, in-place retry of the latest failed question, and crash recovery (Pending → Failed). Context uses only same-Revision Answered or Insufficient-evidence turns. Long Transcripts use lexical retrieval, which folds Vietnamese diacritics and adds neighbours. The Answer text streams from partial JSON. Validation gives Answered, Unverified (invalid or missing Citations), Insufficient evidence (shown as fixed app text, so a model claim can't appear under that label), Off-topic nudge, Incomplete (Stop, or failure after text), or Failed.
- App: the Ask panel streams the Answer and has Stop, Retry (latest failed only), playable Citations, and labels for each state. The cloud consent/disclosure flow matches the Summary's.
- Manual check on the laptop: ask something discussed (you get cited Answers that play), something never mentioned (Insufficient evidence), Stop mid-answer (Incomplete), restart the app (history is kept), and say "hello" (you get a nudge).

### 2026-10-13: laptop check passed

The owner ran the manual checks on the laptop and all passed. Resolved.
