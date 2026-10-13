# Sentence-sized Segments

Status: ready-for-agent
Spec: ../spec.md
Blocked by: 06

## What

Today Whisper's own segments are passed through as they are, so some are too short and some too long. Re-segment instead:

- Enable token or word timestamps in `WhisperEngine::transcribe` and return word-level timing in `RawSegment` (or a new word list). Map the times back through the VAD time map as today.
- A pure function in core (no Whisper dependency) builds Segments from one Source's words:
  - split after sentence-ending punctuation (`.` `?` `!` `…`, and their Vietnamese equivalents);
  - merge a fragment under `MIN_SEGMENT_MS` (about 1 s) or under `MIN_SEGMENT_WORDS` (3) into the closest neighbour of the same Source;
  - split a Segment over `MAX_SEGMENT_MS` (about 20 s) at its longest gap between words, repeating until every part fits;
  - never join across a VAD gap longer than about 2 s, even mid-sentence.
- Bracketed non-speech tags are still dropped.
- `CONTEXT.md` already defines a Segment this way. Do not change it.

## Acceptance criteria

- [ ] Unit tests for the segmenter: sentence split, fragment merge, long split at the longest gap, no join across a long pause, and none across Sources.
- [ ] The benchmark from ticket 03 shows fewer than 5% of Segments under 1 s or over 20 s on the long clip, with WER no worse than before.
- [ ] Segment IDs remain stable for an unchanged Revision (the existing tests pass).
