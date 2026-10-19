# Roadmap backlog after 1.1.0

Status: needs-triage

Decided on 2026-10-19 in a grilling session, informed by a competitor scan (Granola, Otter, Fireflies, tl;dv, Krisp, Jamie, Meetily, Hyprnote, MacWhisper, Notion, Teams, Zoom, Read.ai, Bluedot). Terms are defined in `CONTEXT.md`.

## Positioning

Capture from any calling app on Windows, Meetings kept as local files, and claims cited to audio you can play. Bot-free capture alone is no longer distinctive. The main user is an individual knowledge worker who doesn't want a bot in the call; non-English Meetings are the edge to win on. Sync, sharing and macOS stay out (ADR 0001, 0002, 0013).

## Goal

Make the product better for existing weekly users. Monetisation must come from that work, not lead it. Pro sells only a larger Managed transcription allowance; every app feature stays free, as the Free plan promises.

## 1.2: Never miss a call

Spec: `.scratch/never-miss-a-call/spec.md`.

The most common complaint in the scan is a missed or empty Recording. If capture fails, nothing downstream can help.

- **Record prompt**: when another app starts using the Microphone and no Recording runs, offer Record / Not now / Never for this app. Once each time that app starts using the Microphone. On by default, with a switch in Settings. It never starts a Recording by itself, which keeps the deliberate-start rule from the Windows MVP spec.
- **Source health**: warn on the Capture bar and with a Windows notification, offering "Open sound settings", when the Microphone has delivered pure digital silence for 60 s, or System audio has been silent while the Mic picked up speech for 3 min. Warn only, never stop.
- Ticket `v1-launch/20`: Managed transcription detects the Meeting language on Auto-detect.
- Ticket `v1-launch/01`: the last acceptance box, the window opening without a visible delay after a tray click.

## 1.3: Follow-through

- **Action items** become Open or Done and can be edited or removed. Once the user touches an item it belongs to them: a new Summary proposes items but never overwrites, reopens or brings back changed ones.
- **Open items**: one view of every Open Action item across the Library. No integrations with task services; copying an item with its Citation is enough.
- **Copy minutes**: the Summary to the clipboard as rich text and Markdown.
- **Summary formats**: Standard, 1:1, Interview, Client minutes. Built in, not user-written; every item still needs a Citation.
- Move a selected range of Segments to one Participant in one action. A cheap alternative to diarization.

## 1.4: Library memory

- **Library Ask**: a question about the whole Library, answered from the Meetings found relevant to it. Citations name the Meeting and the Segment; Insufficient evidence still applies. Before sending to a cloud Language model, show the Meetings to be sent and let the user untick some; no dialog for Ollama. Needs an ADR on how the relevant Meetings are found and how many are read.

## Later

- PDF export via WebView print. DOCX and subtitles only if asked for.
- Throwaway prototype of local diarization within Remote. Needs its own ADR, as the Titles and Participants spec says.
- User-written Summary templates.
- Managed Language model access as a possible Pro feature, so Library Ask works without a key of your own (new ADR).

## Considered and set aside

- Live transcript during a Recording: ruled out by ADR 0010.
- Automatic Recording: conflicts with the deliberate-start rule.
- Calendar access: Google needs OAuth and likely a service; the Record prompt covers the forgotten-recording case.
- AI over Snapshots: ADR 0017.
- Apache-2.0 license (`v1-launch/09`): superseded by ADR 0013.
