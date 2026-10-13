# Transcription speed and quality spec

Status: ready-for-agent

Make the Transcript ready within seconds of Stop, and make its Segments sentence-sized, using the user's existing hardware. Vocabulary: `CONTEXT.md` (**Segment**, **Speaker label**, **Draft**, **Revision**, **Chunk**). Decisions: ADR 0008 (local only, Vulkan), ADR 0010 (background transcription).

## Goal

On a Ryzen 7 8745H laptop with a GeForce RTX 4050 Laptop GPU (6 GB) on AC power, the user stops a one-hour call and sees a Draft at once and the finished Transcript within about a minute. Each Segment holds one sentence from one person.

## Evidence (Copy diagnostics, 2026-10-13)

- The transcription backend was `Vulkan (AMD Radeon(TM) 780M)`. The RTX 4050 was never used, because `plan_backend` picks the device with the most memory, and the integrated GPU reports shared system RAM.
- On the 780M the RTF was 0.10–0.14, so a 53-minute Meeting took 6.7 minutes in Whisper.
- One run took 92 minutes and ended in `processing_failed` with no reason logged. Two other Recordings were made while it ran.
- VAD removed only 16–33% of the audio.

## Requirements

### Use the right device
- Prefer a discrete GPU over an integrated one, whatever their reported memory. Among devices of the same type, keep today's rule (most memory, ties in ggml order).
- Use a discrete GPU only on AC power. On battery, use the integrated GPU, or the CPU when there is none. The choice is made when a transcription starts.
- Copy diagnostics names the chosen device and the power state.
- Stay on Vulkan. CUDA is reconsidered (by amending ADR 0008) only if the benchmark shows it more than 1.5× faster than Vulkan on the RTX 4050.

### Explain failures
- `processing_failed` logs the error kind, the stage, and how much audio was transcribed before the failure.
- Processing keeps running while a new Recording is made.

### Measure
- A benchmark set of public English and Vietnamese clips (for example FLEURS or Common Voice) with reference transcripts. The clips are downloaded, not committed.
- A script or ignored test reports WER, RTF and backend for each model and device, plus Segment-length statistics.

### Choose the model automatically
- A new **Auto** model setting, which becomes the default. It picks the best model that fits the device's memory and stays under an RTF of about 0.05 on it, using a table filled in from benchmark results.
- Auto may choose models heavier than turbo q5_0 (for example turbo q8_0 or large-v3) if the benchmark justifies adding them.
- An explicit model in Settings still overrides Auto. Existing explicit choices are kept.

### One stream per Source
- When a Recording has both Sources, transcribe the Microphone and System audio as separate streams, in parallel where the device allows.
- A Segment's Speaker label is the Source it was transcribed from: Me or Remote. Unknown is used only when there is no Source split, as in an Import. A Voice note's Segments are Me.
- No mic-bleed de-duplication (the user wears a headset). Segments from the two Sources may overlap in time.

### Sentence-sized Segments
- A Segment is about one sentence: split at sentence-ending punctuation using word-level timestamps.
- A fragment shorter than about 1 s, or under 3 words, joins the neighbouring Segment of the same Source that is closest in time.
- A Segment longer than about 20 s is split at its longest internal pause.
- A Segment never crosses a change of Speaker label.
- The thresholds are constants that the benchmark can tune.

### Draft
- During Processing, the Meeting view shows finished Segments as a read-only Draft, updated as transcription advances.
- A Draft cannot be cited, asked about or searched, and its Segments have no stable IDs. When Processing completes, the Draft is replaced by the published Revision. When Processing fails or is cancelled, the Draft is discarded.

### Background transcription during a Recording (ADR 0010)
- While a Recording runs on a discrete GPU on AC power, completed Chunks are transcribed in the background.
- Nothing is shown during the Recording. After Stop the Draft appears at once, and only the remaining audio is transcribed.
- Unplugging stops background work and keeps the results. Plugging back in resumes it. Pause lets it finish the audio already captured.
- Background results are a cache outside the Library. After a crash they are discarded, and the Meeting is transcribed from its Chunks.
- Segments are stitched across Chunk edges, so a sentence spoken across an edge becomes one Segment.

### Existing Meetings
- Nothing is re-transcribed automatically. The existing "Re-transcribe" action produces a new Revision under the new rules. Citations keep pointing to the old Revision.

## Out of scope

- A live transcript during a Recording.
- Cloud transcription (ADR 0008).
- Telling several remote voices apart (diarization).
- Mixed-language Meetings. One Meeting language stays the rule.
- Mic-bleed de-duplication for users on laptop speakers.

## Tickets

1. `01-prefer-discrete-gpu-on-ac-power.md`
2. `02-log-processing-failure-reasons.md`
3. `03-transcription-benchmark.md`
4. `04-benchmark-on-rtx-4050.md` (human)
5. `05-auto-model-choice.md`
6. `06-transcribe-each-source-separately.md`
7. `07-sentence-sized-segments.md`
8. `08-draft-during-processing.md`
9. `09-background-transcription-during-recording.md`
