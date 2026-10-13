# Correction before publishing

Status: ready-for-agent
Spec: ../spec.md
Blocked by: 04

## What

- Add a Correction step to Processing after transcription and before the Revision is published. It runs when a Gemini key is set, on any engine's output.
- Request: the Segments (ID, Speaker label, text), the Meeting language, the Vocabulary and the Participant names. The instruction is to fix misheard words, names, terms, punctuation and diacritics only: no rephrasing, summarising or translating.
- Response (JSON schema): `{ segments: [{ id, text }] }`.
- Apply rules:
  - change text only;
  - a Segment the response leaves out, duplicates, returns empty or doesn't know keeps its original text;
  - times, Sources, Speaker labels, IDs and Segment count never change.
- Send one request per Meeting where it fits the model's context. Otherwise batch by consecutive Segments, as few batches as possible.
- On failure, timeout or a quota hit, publish the uncorrected text and show a one-line notice in the Meeting view.
- The Draft stays uncorrected; only the published Revision has the Correction.
- Diagnostics: Correction ran, the outcome, how many Segments changed, duration.

## Acceptance criteria

- [ ] Core tests with a fake provider:
  - text-only change;
  - missing, duplicate and unknown IDs are ignored;
  - the Segment count and times stay identical;
  - failure publishes uncorrected text with the notice;
  - batching beyond the context limit.
- [ ] With no Gemini key, Processing is unchanged.
