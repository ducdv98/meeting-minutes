# Product Hunt launch kit

Updated 2026-10-20 for the 1.2.0 launch. **Launch: Saturday 2026-10-24, scheduled for 00:01 Pacific.**

Decisions behind this version (grilling session 2026-10-20):

- **No "open source" claim.** There is no `LICENSE`, and `docs/research/dependency-licenses.md` lists two blockers (statically linked LGPL LAME encoder; brand artwork rights unverified). Say "free, source on GitHub" only. The Apache-2.0 work (ticket 09) happens after launch.
- **Managed transcription is by hand-issued key** (ADR 0016), not Accounts. No minute count in the copy; the allowance is not measured (ticket 17).
- The tagline stays on the core pitch. "Never miss a call" (Record prompt, Source health) goes in the first comment.
- Goal: feedback and early users, not a ranking.

## Name and tagline

- **Name:** Meeting Minutes
- **Tagline** (60 characters or fewer): `Record any Windows call, get a cited transcript and summary` (59)
- Backup: `Local-first meeting notes for Windows, with cited answers` (57)

## Description

(260 characters or fewer)

> Record Zoom, Teams, Meet or any call on Windows. Meeting Minutes transcribes on your PC with Whisper or with your own keys, in 11 languages, then writes a Summary and answers questions with Citations you can click to hear. Free and local-first.

## Topics

Productivity, Meetings, Artificial Intelligence, Windows (not Open Source)

## Gallery

Images are 1270 × 760, regenerated 2026-10-20 from 1.2.0 with `pnpm screenshots --gallery` (`.scratch/v1-launch/gallery/`), light theme unless noted.

| # | Image | Caption |
|---|---|---|
| 1 | `meeting-light.png` | A Meeting after Processing: Transcript, Participants and a Summary with Citations |
| 2 | `ask-light.png` | Ask one Meeting a question. Every Answer cites the Transcript |
| 3 | `snapshots-light.png` | Snapshot a shared slide mid-call. It lands in the Transcript at that moment |
| 4 | `search-light.png` | Search every Transcript in your Library, in any of 11 languages |
| 5 | `firstrun-model-light.png` | Setup recommends a local model for your PC. Or bring your own key |
| 6 | `library-dark.png` | Your Library stays on your PC |

The Record prompt and Source health are not in the gallery set. If you want them as image 3 or 4, capture them on a real Windows machine and say so.

The thumbnail is the app icon from `assets/brand/`. The maker records a 30–60 second demo video: start a call, record, stop, then show the Summary, click a Citation, ask a question, and finish on search.

## Maker's first comment

> Hi Product Hunt! I'm Duc, and I built Meeting Minutes because I wanted meeting notes I could check, without sending every call to someone else's server.
>
> **How it works:** it records your Microphone and what your PC plays, so it works with Zoom, Teams, Meet or anything else, with no bot joining the call. It transcribes with Whisper on your PC by default, or with your own Groq or Gemini key. Summaries and Answers cite the Transcript, and you can click a Citation to hear that moment.
>
> **New in 1.2.0, never miss a call:** when another app starts using your Microphone, a small prompt offers to record (it never starts on its own). While recording, the app warns you if your Microphone or the other side goes silent. Both checks run on your PC and send nothing anywhere.
>
> **Cloud is optional:** local and your-own-key Processing have no app-imposed limit. If your PC is slow, you can request a Managed transcription key from the website. I issue them by hand while this is experimental.
>
> **Unsigned installer:** I don't pay for a code-signing certificate, so Windows SmartScreen will warn you. Every Release has a SHA-256 checksum, a GitHub build attestation (`gh attestation verify`), and the source is public on GitHub. How to verify: meeting-minutes.tech/support/#smartscreen
>
> **Pro** is planned, with no price yet. Tell me what you'd want in the GitHub Discussion linked from the pricing page.
>
> Windows 11 only for now; a Mac version is in progress. I'd love your feedback. Bug reports go to GitHub Issues from Help → Report a problem.

## Launch-day checklist

**Before Saturday (owner)**
- [ ] Clean-machine check of 1.2.0 on Windows (ticket 16)
- [ ] 1.2.0 is published on GitHub Releases by Thursday 2026-10-22, 1.1.0 updates to it, and the site shows 1.2.0 with the download button
- [ ] SmartScreen note and checksum visible by the download button (done in the site build)
- [ ] The Managed key spending ceiling and the request email are checked for launch traffic
- [ ] The Pro Discussion and the Sponsors or Ko-fi links work
- [ ] The Product Hunt draft is filled in: name, tagline, description, topics, gallery, video, thumbnail, first comment
- [ ] The launch is scheduled for 00:01 Pacific on Saturday 2026-10-24

**Launch day**
- [ ] Post the first comment as soon as the launch goes live
- [ ] Share the launch in your own channels (no vote asking)
- [ ] Answer every comment within the hour
- [ ] Watch the gateway dashboard: Managed key requests and monthly spend against the ceiling
- [ ] Watch GitHub Issues and the support inbox; fix crash or data-loss bugs first

**The day after**
- [ ] Thank commenters, and file the feedback as tickets
- [ ] Record downloads and the Managed transcription cost

## After launch

- Ticket 09: apply Apache-2.0. Resolve the LAME LGPL compliance (or replace the encoder), publish third-party notices with the installer, and settle the brand artwork rights. Then update the README and the site, and extend ADR 0013.
