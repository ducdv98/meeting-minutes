# Sentence-sized Segments

Status: ready-for-human
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
- Laptop check: In a recorded one-hour call, Segments read as single sentences, none longer than about 20 s, and no fragment like "court with" stands alone.

## Comments

2026-10-13, same CPU smoke run after this ticket (and the follow-up that caps a word at `MAX_WORD_MS` and avoids fragment splits):

| Model | Backend / device | Set | WER | RTF | After VAD | Median s | p10 s | p90 s | <1 s | >20 s |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| base | CPU | English short (10) | 10.3% | 0.195 | 86.3s | 6.9 | 4.1 | 9.3 | 0% | 0% |
| base | CPU | Vietnamese short (10) | 43.6% | 0.181 | 89.0s | 7.9 | 4.7 | 12.4 | 0% | 0% |
| base | CPU | English stitched (1) | 69.7% | 0.087 | 382.0s | 11.2 | 4.6 | 18.4 | 0% | 0% |

The stitched clip had 36% of Segments over 20 s before this ticket, and 0% under 1 s or over 20 s now; WER is unchanged. The first version of the segmenter left 13% under 1 s: Whisper timed a sentence's last word across the 3 s silence after it ("faces." lasted 26 s), and the 20 s split then cut that sentence into fragments.
