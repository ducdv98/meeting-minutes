# Windows and macOS desktop app

> Supersedes ADR 0001. Amends ADR 0005.

Meeting Minutes was built as an independent Windows app (ADR 0001). We now also ship it on macOS, as a **macOS Preview** for Apple Silicon on macOS 14.4 or later. The core workflow stays Record → Transcribe → Ask with Citations on both platforms. Platform code stays thin: core owns the shared behaviour, and each platform only supplies adapters for what the OS does differently (audio capture, the clock, secrets, the trash, power state, screen capture, revealing files).

ADR 0005's rule becomes "every Source on the host clock" instead of "every Source on QPC". Core keeps one monotonic host clock (in 100 ns ticks, as today) and does the alignment (pad gaps with silence, drop overlapping frames, no resampling) for every platform. WASAPI frames are timed with QueryPerformanceCounter. Core Audio frames are timed with `AudioTimeStamp.mHostTime`. System audio on macOS comes from a Core Audio process tap of every process except Meeting Minutes itself.

## Considered Options

- **Stay Windows-only**: no extra platform to maintain, but leaves Mac users out, even though most of the code (the UI, the Library, Ask and the cloud providers) already builds on any platform.
- **ScreenCaptureKit audio, macOS 13+**: works on more Macs, but even audio-only use needs the Screen Recording permission, and it is a second capture path next to process taps.
- **Intel Macs too**: Whisper is too slow on them to be usable, and supporting them would double the build and test matrix.
- **Resample Sources on macOS**: Mac Sources often run on different device clocks (AirPods, USB headsets). We keep no-resampling until the M4 measurement shows more than about 100 ms of relative drift per hour.

## Consequences

- At first the macOS Preview has no Record prompt (macOS has no public API that says which app is using the Microphone) and no background transcription (ADR 0010's discrete-GPU rule never holds on Apple Silicon). Snapshots arrive after the first Preview build.
- On macOS the Capture bar lives in the menu bar, because a floating window can't be reliably kept out of ScreenCaptureKit-based screen sharing.
- A Recording keeps the Mac out of App Nap and idle sleep. Closing the lid still sleeps the Mac.
- On macOS, the Library is `~/Documents/Meeting Minutes`, and macOS asks once for Documents access.
- One `vX.Y.Z` tag builds both platforms with the same version and updater key. A failed macOS job never blocks the Windows release.
- Mac capture and permission work needs real hardware (an M4). CI builds and tests on `macos-14` but cannot test capture.
