# Ask a Meeting with cited Answers

Status: ready-for-agent
Blocked by: 10, 12
Spec: ../spec.md

## What

Ask panel on the Meeting view following the spec's Ask rules: scoped to the current Revision, retrieval of relevant Segments (with neighbours) when the Transcript exceeds the model context, structured Answer `{answer, citations[], insufficient_evidence}`, citation validation, streaming, Stop, Incomplete/Unverified states, one active Ask per Meeting, retry for the latest failed question, history in `asks.json`, nudge for off-topic input.

## Acceptance criteria

- [ ] Question with support → Answer with clickable Citations that play the right span.
- [ ] Question without support → Insufficient evidence, no invented content.
- [ ] Stop mid-stream → partial text labelled Incomplete and excluded from later context.
- [ ] History survives restart; a failed question can be retried and is not duplicated.
- [ ] Unit tests: citation validation (unknown, cross-Meeting, other-Revision IDs rejected), context building, state transitions.

## Comments
