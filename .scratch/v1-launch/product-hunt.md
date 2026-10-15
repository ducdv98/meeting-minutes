# Product Hunt launch kit

Draft for ticket 18. Two things are still open: **N** (the free Managed transcription minutes a month, set by ticket 17) and the **launch date**, a Saturday about two weeks after 0.9.0 ships. Say "open source" only once the Apache-2.0 LICENSE from ticket 09 is in the repo.

## Name and tagline

- **Name:** Meeting Minutes
- **Tagline** (60 characters or fewer): `Record any Windows call, get a cited transcript and summary` (59)
- Backup: `Local-first meeting notes for Windows, with cited answers` (57)

## Description

(260 characters or fewer, 251 here)

> Record Zoom, Teams, Meet or any call on Windows. Meeting Minutes transcribes locally with Whisper or with your own keys, in 11 languages, then writes a Summary and answers questions with Citations you can click to hear. Free, open source, local-first.

## Topics

Productivity, Meetings, Artificial Intelligence, Open Source, Windows

## Gallery

Images are 1270 × 760, from `pnpm screenshots --gallery` (`.scratch/v1-launch/gallery/`), in light theme unless noted.

| # | Image | Caption |
|---|---|---|
| 1 | `meeting-light.png` | A Meeting after Processing: Transcript, Participants and a Summary with Citations |
| 2 | `ask-light.png` | Ask one Meeting a question. Every Answer cites the Transcript |
| 3 | `search-light.png` | Search every Transcript in your Library, in any of 11 languages |
| 4 | `transcript-dark.png` | Timed Transcript with Participant names. Click a line to hear it |
| 5 | `firstrun-model-light.png` | Setup recommends a local model for your PC. Or bring your own key |
| 6 | `library-dark.png` | Your Library stays on your PC |

The thumbnail is the app icon from `assets/brand/`. The maker records a 30–60 second demo video: start a call, record, stop, then show the Summary, click a Citation, ask a question, and finish on search.

## Maker's first comment

> Hi Product Hunt! I'm Duc, and I built Meeting Minutes because I wanted meeting notes I could check, without sending every call to someone else's server.
>
> **How it works:** it records your Microphone and what your PC plays, so it works with Zoom, Teams, Meet or anything else, with no bot joining the call. It transcribes with Whisper on your PC by default, or with your own Groq or Gemini key. Summaries and Answers cite the Transcript, and you can click a Citation to hear that moment.
>
> **Free cloud minutes:** if your PC is slow, sign in with your email and get N minutes of Managed transcription a month, admitted while capacity lasts. Local and your-own-key Processing are unlimited and always free.
>
> **Open source and unsigned:** the app is Apache-2.0 on GitHub. I don't pay for a code-signing certificate, so Windows SmartScreen will warn you. Every Release has a SHA-256 checksum, a GitHub build attestation (`gh attestation verify`) and a VirusTotal link, or you can build it yourself. Details: meeting-minutes.tech/support/#smartscreen
>
> **Pro** is coming soon. If you'd pay for it, tell me in the GitHub Discussion linked from the pricing page.
>
> Windows 11 only for now. I'd love your feedback, and bug reports go straight to GitHub Issues from Help → Report a problem.

## Launch-day checklist

**The week before**
- [ ] 1.0.0 is published, and 0.9.0 updates to it automatically (ticket 19)
- [ ] The site shows 1.0.0 with the download button, pricing with the final N, and no Beta, roadmap or dates
- [ ] LICENSE is in the repo, and the site and copy may say "open source" (ticket 09)
- [ ] The Managed transcription ceiling and admission count are set for launch traffic; the waitlist email works
- [ ] The Pro Discussion and the Sponsors or Ko-fi links work
- [ ] The Product Hunt draft is filled in: name, tagline, description, topics, gallery, video, thumbnail, first comment
- [ ] The launch is scheduled for 00:01 Pacific on the launch Saturday

**Launch day**
- [ ] Post the first comment as soon as the launch goes live
- [ ] Share the launch in your own channels (no vote asking)
- [ ] Answer every comment within the hour
- [ ] Watch the gateway dashboard: signups, admitted vs waitlisted, monthly spend against the ceiling
- [ ] Watch GitHub Issues and the support inbox; fix crash or data-loss bugs first

**The day after**
- [ ] Thank commenters, and file the feedback as tickets
- [ ] Record signups, downloads and the Managed transcription cost
