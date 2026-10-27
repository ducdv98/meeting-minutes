# Capture on the QPC clock, encode with hard clipping

> Amended by ADR 0020: the rule is now "every Source on the host clock". QPC is the Windows host clock, and Core Audio host time is the macOS one.

Each Source is placed on the shared QueryPerformanceCounter clock by inserting silence for gaps and dropping overlapping frames, as the capture spike did. We do not resample. The laptop run in ticket 03 measured both endpoints at +16.6 ppm against QPC (about 60 ms per hour) and no measurable drift between Microphone and System audio. Keeping each Source on QPC therefore keeps them aligned with each other and with the Timeline, at the cost of an occasional dropped or padded frame.

Encoding hard-clips samples to [-1, 1]. The Microphone peaked at +2.17 dBFS in float, so some clipping happens on loud speech, but we accept it rather than add a limiter or gain stage. The Microphone is stored mono, because its two channels were sample-identical. FLAC is 16-bit, 48 kHz. The mixed `meeting.mp3` is 48 kHz stereo at 128 kbps CBR from LAME (`mp3lame-encoder`, LGPL, statically linked; acceptable for a personal app that is not distributed commercially).

## Considered Options

- **Resample each Source to QPC**: removes the frame drops but adds a resampler to the capture path for an error we measured at 60 ms per hour.
- **Limiter or automatic gain before encoding**: avoids clipping, but changes levels the user did not ask for. We can revisit this if transcription quality suffers.

## Consequences

- Revisit if a longer call or a different device (e.g. separate mic and speakers on different clocks) shows relative drift between Sources.
