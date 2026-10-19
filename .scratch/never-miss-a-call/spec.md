Status: ready-for-agent

# Never miss a call

## Problem Statement

The worst thing that can happen to a user of Meeting Minutes is losing a call: they forgot to press Record, or they did press it but one Source captured nothing (a muted or wrong Microphone, System audio coming out of a device the app isn't listening to). Either way they find out after the meeting, when the Transcript is missing or one-sided, and no Summary or Ask can bring it back. Today the app only shows a small "Silent" tag next to a Source in the main window's recording bar, which the user rarely sees because they are in the meeting app. The Capture bar shows nothing about Source health, and nothing reminds the user to record when a call starts.

## Solution

Two safeguards, both only offering or warning, never acting on their own:

- **Record prompt**: when another app starts using the Microphone and no Recording runs, Meeting Minutes shows a small always-on-top prompt naming that app, with **Record**, **Not now** and **Never for this app**. It appears once each time that app starts using the Microphone. Record starts a Recording exactly as the main window's Record button does. The prompt can be turned off in Settings, and the user can manage the apps they silenced.
- **Source health**: while a Recording runs, the app watches each Source. When the Microphone has delivered pure digital silence for 60 seconds, or System audio has been silent for 3 minutes while the Microphone heard speech, the Capture bar and the main window's recording bar show a warning for that Source, and Windows shows a notification. The warning offers **Open sound settings** (the Recording follows a change of the Windows default device and marks it on the Timeline) and **Dismiss**. The Recording never stops or pauses because of a warning.

## User Stories

1. As a user joining a Teams call, I want Meeting Minutes to offer to record when Teams starts using my Microphone, so that I don't forget to press Record.
2. As a user joining a Google Meet call in my browser, I want the prompt to appear for the browser too, so that web-based calls are covered.
3. As a user, I want the prompt to name the app that started using the Microphone, so that I understand why it appeared.
4. As a user, I want to press Record on the prompt and have the Recording start straight away, so that I don't have to switch to the main window.
5. As a user who has never seen the Recording notice, I want Record on the prompt to show the Recording notice first, so that I'm still reminded about consent before my first Recording.
6. As a user, I want Record on the prompt to use my usual default Meeting language and settings, so that a prompted Recording is the same as one started from the main window.
7. As a user, I want Not now to close the prompt without recording, so that I can ignore calls I don't want to keep.
8. As a user, I want the prompt to close by itself after a short time if I ignore it, counting as Not now, so that it doesn't sit over my meeting.
9. As a user, I want the prompt to appear only once each time an app starts using the Microphone, so that it doesn't nag me during a call I chose not to record.
10. As a user, I want to be prompted again the next time that app starts using the Microphone, so that a Not now applies to one call only.
11. As a user who plays games or uses dictation, I want Never for this app, so that apps I never record stop prompting me.
12. As a user, I want to see and remove the apps I silenced in Settings, so that I can undo a Never for this app.
13. As a user, I want to turn the Record prompt off completely in Settings, so that I'm in control of what the app does in the background.
14. As a user, I want the Record prompt on by default, so that I benefit from it without setting anything up.
15. As a user, I want no prompt while a Recording is already running, so that I'm not asked to record a call I'm recording.
16. As a user, I want no prompt for Meeting Minutes' own use of the Microphone, so that the app never prompts about itself.
17. As a user, I want no prompt when several apps already use the Microphone and one of them stops, so that only a new use of the Microphone counts.
18. As a user, I want the prompt never to start a Recording by itself, so that nobody is recorded without me deciding to.
19. As a user, I want the prompt to stay out of screen sharing and Snapshots, like the Capture bar, so that other participants never see it.
20. As a user, I want the prompt to work while the main window is closed and the app sits in the tray, so that it helps when I'm not looking at the app.
21. As a user recording a call, I want to be warned when my Microphone has delivered no signal at all for a minute, so that I can fix a muted or wrong Microphone before the call is over.
22. As a user recording a call, I want to be warned when System audio has been silent for three minutes while I've been talking, so that I notice when the other side isn't being captured.
23. As a user, I want no warning when System audio is simply quiet because nobody else has joined yet and I'm not talking, so that waiting rooms don't set off alarms.
24. As a user, I want no Microphone warning when my Microphone is only quiet (room noise but nobody speaking), so that a quiet listener isn't told something is wrong.
25. As a user, I want the warning shown on the Capture bar, so that I see it without leaving the meeting app.
26. As a user, I want the warning also shown in the main window's recording bar, so that it is consistent wherever I look.
27. As a user, I want a Windows notification for the warning, so that I notice it even when the Capture bar is collapsed or the main window is hidden.
28. As a user, I want the warning to say which Source is affected and the device it is capturing from, so that I know what to fix.
29. As a user, I want Open sound settings in the warning to open Windows Sound settings, so that I can change the default device quickly.
30. As a user, I want the Recording to keep going after a warning, so that I never lose audio because of a false alarm.
31. As a user, I want the warning to clear by itself once the Source carries sound again, so that I'm not left looking at a stale warning.
32. As a user, I want Dismiss to hide a warning for this Recording, so that a known silent Source (I'm only listening) stops bothering me.
33. As a user, I want a dismissed warning to come back only after a Source recovers and fails again, so that a new problem is still reported.
34. As a user, I want no Source health warning while the Recording is paused, and the time spent paused not to count, so that pauses don't trigger alarms.
35. As a user who switches headsets mid-call, I want the silence timers for a Source to restart after its device switch, so that I'm judged on the new device.
36. As a user recording a Voice note, I want no System audio warning, so that a Microphone-only Meeting is not flagged for missing remote audio.
37. As a user, I want the existing short "Silent" tag next to each Source to keep working, so that I still see instant feedback on the level meters.
38. As a user, I want Source health warnings to stay on my PC and never appear in diagnostics with any audio content, so that privacy is unchanged.

## Implementation Decisions

- **Record prompt decider (new core module).** A pure, deep module that turns observations into decisions. Its input on each check is the set of other apps currently using the Microphone (each identified by a stable app ID plus a display name), whether a Recording is active, and the prompt settings (on/off and the silenced app IDs). Its output is at most one prompt to show (app ID and display name), or an instruction to close an open prompt. It remembers which apps were already using the Microphone, so a prompt only fires for an app that was not using it at the previous check. An app that started while a Recording was active doesn't prompt later in the same Microphone session. The first check after the app starts treats apps already using the Microphone as old, so launching Meeting Minutes mid-call never prompts. Meeting Minutes' own process is excluded by the caller.
- **Microphone usage (new trait).** A small interface that returns the apps using the Microphone right now. The Windows implementation reads the per-app Microphone consent store Windows keeps for its privacy indicator (an app is in use while its last-used stop time is zero), covering both packaged and desktop apps, and derives the display name from the package name or executable name. It is polled on a slow interval (about 2 seconds) by the Tauri layer, which feeds the decider. There is no automated test for the Windows implementation.
- **Record prompt window.** A small always-on-top webview window like the Capture bar, excluded from capture and screen sharing the same way, positioned near the tray corner, closing after about 20 seconds as Not now. Record calls the same start command as the main window, so the Recording notice, Meeting language and defaults behave identically. If the Recording notice hasn't been acknowledged, Record opens the main window on the notice instead of starting.
- **Settings.** Two new settings: Record prompt enabled (default on) and the list of silenced apps (ID and display name). Settings' Recording section gains the switch and the list with Remove. Older settings files without these fields load with the defaults.
- **Source health (extends the recorder).** Source health is part of the recorder's status, next to the existing Source levels. Each Source reports a health state: OK, or a warning with its kind: *no signal* (Microphone only: every packet for 60 s was digital silence, meaning flagged silent by the device or all-zero samples, not just low level) or *remote silent* (System audio below the audible threshold for 3 min while the Microphone was at speech level for a meaningful share of that window). The existing audible tracking per Source is the basis. Paused time doesn't count toward either window, and a device switch on a Source restarts its windows. A Voice note (Microphone only) never reports remote silent. Thresholds are constants in core, not settings.
- **Dismiss.** Dismissing is per Source and per Recording. A dismissed warning stays hidden until that Source returns to OK and then fails again. Dismissal state lives in the recorder so the Capture bar and main window agree.
- **Notification.** When a Source enters a warning state that isn't dismissed, the Tauri layer shows one Windows notification for it, with the Source name and device. This needs a notification plugin added to the Tauri app; clicking the notification opens the main window. The Capture bar and the main window's recording bar render the warning from the recorder status, with **Open sound settings** (opens Windows Sound settings) and **Dismiss**.
- **No change** to capture, Chunks, the Timeline or Markers. A warning is not a Marker and isn't saved to the Meeting.
- **Diagnostics** may count Source health warnings and Record prompts shown per session, without app names, devices or audio content.

## Testing Decisions

- A good test checks external behaviour only: what the recorder status reports and what the decider returns for a series of observations, and what the rendered views show, not internal fields.
- **Seam 1, recorder status** (`crates/core/tests/recorder.rs`), using the existing fake clock and fake input openers, with packets of chosen level and silent flag. Cover:
  - no signal after 60 s of digital silence on the Microphone, and not after 60 s of low-level noise
  - remote silent after 3 min of System audio silence while the Microphone is at speech level, and not when both are silent
  - no warnings while paused, with paused time not counted
  - timers restarting after a device switch
  - no remote silent in a Voice note
  - recovery clearing the warning
  - dismiss hiding a warning until recovery and a new failure
- **Seam 2, Record prompt decider** (new `crates/core/tests/record_prompt.rs`), fed sequences of Microphone-usage observations. Cover:
  - a new app prompts once
  - an app continuing to use the Microphone doesn't prompt again
  - the same app stopping and starting again prompts again
  - silenced apps and the off switch never prompt
  - no prompt while a Recording is active, and none later in that same Microphone session
  - apps already using the Microphone at the first check don't prompt
  - a second app starting while one is in use prompts for the second only
  - an open prompt closes when its app stops using the Microphone or a Recording starts
- **Seam 3, UI with mocked IPC**: extend `CaptureBarWindow.test.tsx` and `RecordingBar.test.tsx` for the warning, Open sound settings and Dismiss; `SettingsView.test.tsx` for the switch and the silenced apps list; a new test for the prompt window's three buttons and the notice-first path.
- **No automated tests** for the Windows-native layer. Manual checklist:
  - Teams (packaged), Zoom (desktop) and Meet in Edge and Chrome each prompt once
  - Discord and Windows dictation can be silenced
  - no prompt for the app itself
  - the prompt is absent from a shared screen and from Snapshots
  - a hardware-muted or privacy-blocked Microphone gives no signal within about a minute
  - playing call audio to a headset while the default output is the speakers gives remote silent, or the app follows the default instead
  - the notification appears with the main window hidden
  - Open sound settings opens the right page
- Prior art: `crates/core/tests/recorder.rs` and `recording.rs` for the fakes, `snapshots.rs` for a new core seam with an injected platform trait, and `CaptureBarWindow.test.tsx`, `RecordingBar.test.tsx` and `SettingsView.test.tsx` for UI tests.

## Out of Scope

- Starting a Recording automatically, or recording before the user presses Record.
- Calendar access or reminders for scheduled meetings.
- Prompting on System audio activity or on a meeting app being launched without using the Microphone.
- An in-app device picker, or switching the captured device from the warning.
- Recording only one app's audio (per-app loopback).
- Detecting a one-sided call after Stop, or warnings on Imports.
- Changing the existing 3-second "Silent" tag.
- Auto-repair: restarting capture or changing devices on the user's behalf.
- Platforms other than Windows.

## Further Notes

- The same 1.2 release also carries `.scratch/v1-launch/issues/20-managed-detected-language.md` (Managed transcription detects the Meeting language) and the last open box of `.scratch/v1-launch/issues/01-tray-click-opens-window.md` (no visible delay). They keep their own acceptance criteria.
- The riskiest part is reading which apps use the Microphone on Windows across packaged and desktop apps and browsers. Prototype it first on a real machine before building the decider's UI.
- The Record prompt keeps the deliberate-start rule from the Windows MVP spec: only the user's press starts capture. The Recording notice and the privacy page should say that the app checks which apps use the Microphone, locally, to offer a Recording, and sends nothing.
- Terms are defined in `CONTEXT.md`: Record prompt, Source health, Recording notice, Source, Voice note, Capture bar, Marker.
- Backlog context: `.scratch/roadmap/backlog.md`.
