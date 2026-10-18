Status: ready-for-agent

# Snapshots

## Problem Statement

During a meeting, other participants often share their screen to show a slide, a demo or a document. The Transcript holds what was said, but not what was shown, so afterwards the user cannot tell what "this one here" or "as you can see" referred to. Today the user would have to take a screenshot with another tool and match it to the Meeting by hand.

## Solution

While a Recording runs, the user can take a **Snapshot** of a **Capture region** of their screen with one press, from a floating **Capture bar** or a global hotkey, without switching to the app. Each Snapshot is anchored to the Timeline and appears inline in the Transcript at that moment. The user can open it at full size inside the app or reveal it in its folder. Snapshots are manual, stay on the user's PC, and are not read by Summary, Ask or Citations (ADR 0017).

## User Stories

1. As a user in a call, I want to take a Snapshot with a global hotkey, so that I do not have to leave the meeting app.
2. As a user in a call, I want to take a Snapshot from a button on a floating Capture bar, so that I can capture without remembering a hotkey.
3. As a user, I want to drag a Capture region on screen, so that only the shared content is captured and not my other windows.
4. As a user, I want later Snapshots in the same Recording to reuse my last Capture region, so that capturing a changing slide takes one press.
5. As a user, I want to draw a new Capture region at any time, so that I can follow shared content that moves or changes size.
6. As a user, I want the first Snapshot to ask me for a region when none exists yet, so that I never capture the wrong area.
7. As a user, I want the Capture region to be forgotten when the Recording ends, so that a stale region is never applied to a later Meeting.
8. As a user, I want pause, stop, Snapshot and a timer on the Capture bar, so that I can control the Recording without switching windows.
9. As a user, I want the Capture bar to be always on top and draggable, so that it stays reachable and never blocks what I am looking at.
10. As a user, I want to collapse the Capture bar, so that it takes little space.
11. As a user, I want the Capture bar to appear only while a Recording runs, so that it is not in the way otherwise.
12. As a user, I want starting a Recording to stay in the main window, so that I choose the Meeting and Title deliberately.
13. As a user who shares my own screen, I want the Capture bar and the region overlay never to appear in a Snapshot or in screen sharing, so that participants do not see my controls.
14. As a user, I want a brief on-screen confirmation when a Snapshot is taken, so that I know it worked and nothing is captured silently.
15. As a user, I want the Recording notice to mention Snapshots, so that I remember others' shared content may be captured.
16. As a user, I want to take a Snapshot while the Recording is paused, so that I can pause to show something private and still keep a Snapshot.
17. As a user, I want a Snapshot taken during a pause to sit at the pause Marker, so that the Timeline stays correct.
18. As a user, I want several Snapshots in one pause to keep their capture order, so that they read in the order I took them.
19. As a user, I want Snapshots to be available only while a Recording runs, so that a Snapshot always reflects the live meeting.
20. As a user, I want each Snapshot to appear as a thumbnail in the Transcript between the Segments around it, so that I see what was on screen when something was said.
21. As a user, I want to click a thumbnail to open the Snapshot at full size in the app, so that I can read small text.
22. As a user, I want an "open folder" action on a Snapshot, so that I can use the file in other tools.
23. As a user, I want to add a caption to a Snapshot after the meeting, so that I can label it without interrupting the call.
24. As a user, I want to edit a caption later, so that I can correct it.
25. As a user, I want captions to be found by Library search, landing on the nearest Segment, so that I can find a Snapshot by its label.
26. As a user, I want to delete a single Snapshot, so that I can remove a capture that shows something private or is not useful.
27. As a user, I want to undo a Snapshot deletion, so that a mistaken click does not lose it.
28. As a user, I want Snapshots saved to disk the moment they are taken, so that a crash does not lose them.
29. As a user, I want surviving Snapshots to join the Meeting after a crash, so that the restart adds to the same Meeting, like Chunks do.
30. As a user, I want the app to ask for a new region after a crash restart, so that a lost region cannot capture the wrong area.
31. As a user, I want Snapshots stored as readable PNG files in the Meeting folder, so that I can use the Library without the app.
32. As a user, I want the app to rebuild its Snapshot index from the Meeting folder, so that the Library stays the source of truth.
33. As a user, I want deleting a Meeting to delete its Snapshots, so that nothing is left behind.
34. As a user, I want Snapshots at native pixel resolution on any display scaling, so that slide and code text stays readable.
35. As a user with several monitors of different scale, I want the region captured correctly on each, so that the image matches what I drew.
36. As a user, I want to see the total size of a Meeting's Snapshots, so that I understand its disk use.
37. As a user, I want to change the Snapshot hotkeys in Settings, so that they do not clash with other apps.
38. As a user, I want to be told when a hotkey cannot be registered, so that I know to pick another instead of failing silently.
39. As a user, I want hotkeys to work only while a Recording runs, so that they do not interfere with other apps the rest of the time.
40. As a user, I want my Snapshots to stay on my PC, so that screen content is never sent to the cloud.
41. As a user, I want Summary and Ask to ignore Snapshots, so that answers stay based on the Transcript and Citations only point to Segments.
42. As a user, I want Snapshots to be included in the Meeting folder's export, with image references in the exported transcript, so that shared content travels with the Meeting.

## Implementation Decisions

- New domain terms **Snapshot**, **Capture region** and **Capture bar** are already in `CONTEXT.md`; the decision is recorded in ADR 0017. Use these terms in code and UI. Do not call a Snapshot a screenshot or a Marker.
- **Core Snapshot store.** A new core module owns Snapshot rules and is the main seam. It takes an injected screen-grabber trait (given a region, returns pixels), in the same style as the existing injected clock and audio-input opener, so the rules run without a real screen.
  - Taking a Snapshot: given the current Recording state and Timeline position, grab the region, encode it as PNG, write it durably into the Meeting's folder, and record its Timeline position and capture order.
  - While paused, the Snapshot's position is that of the pause Marker; Snapshots in one pause keep capture order.
  - Snapshots are refused when no Recording is running and for Imports.
  - Caption: optional text stored beside the image, editable at any time.
  - Delete moves the Snapshot to the existing recycle behaviour so that undo restores it with its position and caption.
  - Deleting a Meeting removes its Snapshots.
  - The Snapshot index is derived from the Meeting folder and can be rebuilt from it alone; the database holds only a rebuildable index. The folder layout (a Snapshots folder with a PNG and a small sidecar for position, order and caption) must stay readable by a person.
  - After a crash, written Snapshots are adopted into the Meeting with the surviving Recording data.
  - Report the total size of a Meeting's Snapshots.
- **Capture region** is held for the running Recording only, owned by the app layer, and is not stored in the Meeting. Drawing a region is an explicit action; a Snapshot with no region asks for one first.
- **Native Windows layer** (thin wrappers over the core seam):
  - screen grab of a region, correct for per-monitor DPI scaling and mixed scales;
  - a full-screen region-selection overlay;
  - the always-on-top, draggable, collapsible Capture bar, shown only while a Recording runs;
  - global hotkeys registered only while a Recording runs.
  The Capture bar and the overlay are excluded from capture and from screen sharing using the Windows display-affinity facility.
- **Capture bar** shows pause/resume, stop, Snapshot and the elapsed time, reusing the existing recording state and commands. Starting a Recording stays in the main window.
- **Hotkeys**: one repeats the last region (or draws one if none exists), one always draws a new region. Defaults `Ctrl+Alt+S` and `Ctrl+Alt+Shift+S`, configurable in Settings. A registration failure is shown to the user. Hotkey settings live with the existing recording hotkeys.
- **Confirmation**: a brief on-screen confirmation after each Snapshot, which is also excluded from capture and screen sharing.
- **Recording notice** text gains a line saying Snapshots may capture what others share.
- **Meeting view**: Snapshots render as thumbnails inline in the Transcript at their Timeline position, relative to the Segments around them. Clicking opens a full-size in-app viewer with the caption editor, an "open folder" action and delete with undo. No separate gallery. The Meeting view shows the Snapshots' total size.
- **Library search** indexes captions as text that lands on the nearest Segment. Summary, Ask and Citations are unchanged and do not read Snapshots.
- **Export**: the Meeting folder includes the images, and the transcript export lists them as image references with captions.
- **Format**: PNG at native pixel resolution, no cap on the number of Snapshots.

## Testing Decisions

- A good test checks external behaviour only: what the Library folder contains and what the public API or the rendered view shows, not internal structure.
- **Seam 1, core Snapshot store** (`crates/core/tests/snapshots.rs`), using a fake screen-grabber that returns known pixels and the fake clock used in the existing recording tests. Cover: taking a Snapshot and its position on the Timeline; the position and order for Snapshots taken while paused; refusal outside a Recording and for Imports; durable write and adoption after a simulated crash; caption set and edit; delete and undo; rebuilding the index from the folder alone; Meeting deletion removing Snapshots; total size; caption text appearing in Library search results.
- **Seam 2, Meeting view UI** (extend `MeetingView.test.tsx` / `TranscriptView.test.tsx` with mocked IPC): thumbnail rendered at its position between Segments, full-size viewer opens, caption edit, delete with undo, "open folder" invoked, Recording notice wording, and Settings showing hotkeys and registration failure.
- **No automated tests** for the Windows-native layer. Cover it with a manual checklist: region drag on a single and a mixed-DPI multi-monitor setup; the first Snapshot asking for a region; the repeat hotkey; the Capture bar and overlay absent from the Snapshot; the Capture bar absent when sharing the whole screen in a real meeting app; hotkey clash handling; Snapshot while paused.
- Prior art: `crates/core/tests/recording.rs`, `recorder.rs` and `library.rs` for fakes and Library folder assertions; `MeetingView.test.tsx`, `RecordingBar.test.tsx` and `SettingsView.test.tsx` for UI tests.

## Out of Scope

- Automatic or timed capture, and capture on screen change.
- Sending Snapshots to a vision model, OCR or descriptions; citing Snapshots; Summary and Ask reading images (needs its own ADR).
- Capturing a chosen window or a whole monitor as a mode of its own.
- Snapshots for Imports, or attaching existing image files to the Timeline.
- Moving, cropping or annotating Snapshots.
- A separate gallery view.
- Recording video or screen recording.
- Starting a Recording from the Capture bar.
- Platforms other than Windows.

## Further Notes

- The Capture bar overlaps the existing recording controls in `RecordingBar`; reuse its recording state and commands rather than adding a second source of truth.
- The riskiest parts are the native ones (per-monitor DPI, the capture exclusion, an always-on-top bar). Prototype them first.
- Privacy copy (the privacy page and the Recording notice) should say that Snapshots stay on the PC.
