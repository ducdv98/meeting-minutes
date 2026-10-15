# v1.0.0 and Product Hunt launch

Status: ready-for-agent

Ship Meeting Minutes 1.0.0 for Windows and launch it on Product Hunt. The goal is users and feedback, plus a credible pricing story; there is no revenue at launch. Vocabulary: `CONTEXT.md`. Decisions: ADR 0013 (open-source app, private service), ADR 0014 (Account allowance), ADR 0015 (email-code sign-in, automatic admission, Invite codes retired).

## Scope

Windows only. The installer stays **unsigned**: the app is free and open source, and users can verify the build or build it themselves. Live at 1.0: local Processing, the user's own provider keys, and Accounts with a free Managed transcription allowance. Pro is shown as coming soon. There is no Mac app and no dated roadmap.

## Requirements

### Accounts and Managed transcription (critical path)
- Gateway work lives in the gateway repo, in `meeting-minutes-transcription-gateway/.scratch/accounts/`. It covers Accounts with several installations each, sign-in by a 6-digit email code, metering in Processed audio minutes per calendar month in UTC, allowance enforcement, a calendar-month operator ceiling, automatic admission with a waitlist, signup abuse limits, Delete account, and retiring Invite codes.
- In the app, the Invite code field becomes email-code sign-in. Settings shows remaining minutes, the reset date, and whether the Account is admitted or waitlisted. Delete account is available.
- The allowance is provisionally 120 Processed audio minutes a month during 0.9.0. The final number is published before Product Hunt.

### App polish
- **Tray:** left-click and double-click open and focus the window. Right-click shows the menu.
- **Local model setup:** a skippable first-run flow in three steps: the Recording notice, the model, then the Library location. The model step recommends a model for the hardware, with Download now or "I'll use my own key". The VAD model downloads together with it, and recording is allowed during the download. Settings gets Download and Delete buttons per model. Processing waits and shows "Waiting for model download (x%)". The words "Downloads when first used" go away.
- **Languages:** English, Vietnamese, Spanish, French, German, Portuguese, Italian, Japanese, Korean, Chinese and Hindi. New Meetings default to Auto-detect, with a per-Meeting override. The Summary and Answers are written in the Meeting language. Library search handles accented characters in every language. Setup recommends turbo for non-English languages when the hardware allows it. The app interface stays in English.
- **Report a problem:** a button that opens a prefilled GitHub Issue, with diagnostics copied for the user to paste. No telemetry.
- **Clean-machine bug bash:** a pass on a fresh Windows machine. Crash and data-loss bugs block the launch.

### Cleanup
- Remove "Meetless" from the working tree: rewrite ADR 0001 neutrally, and remove it from the other ADRs, research docs and `.scratch`. Git history stays as it is.
- Remove "Beta" from the app (the About channel, the badge, the diagnostics header), the README, the visual identity doc and the mockup.
- Delete the roadmap section and every target date, on the site and in the README.
- Add an Apache-2.0 LICENSE after a dependency license check. Only then may the site and launch copy say "open source".

### Website and pricing
- Pricing has three cards:
  - **Free:** everything local and with your own keys, plus an Account with N free cloud minutes a month, admitted while capacity lasts.
  - **Pro:** coming soon, linking to a GitHub Discussion ("I'd pay for Pro").
  - **Support the project:** GitHub Sponsors or Ko-fi.
- The privacy page describes Accounts as current behaviour.
- Screenshots come from `pnpm screenshots` and fictional demo data. The first-run model screenshot is captured once that screen exists.

### Release
- Every Release gets a SHA-256 checksum, GitHub artifact attestation from `release.yml`, and a VirusTotal link. The support page and the Product Hunt copy explain the unsigned installer.
- Release 0.9.0 to a small group, measure the Managed transcription cost, and set the allowance. Release 1.0.0 at least a week later, through the auto-updater.
- Product Hunt kit: tagline, description, gallery images at 1270 × 760, the maker's first comment, and a launch-day checklist. The maker records the demo video. Launch on a weekend about two weeks after 0.9.0.

## Order

Gateway Accounts and app polish run in parallel. Both must be done for 0.9.0. Then measure, finish the launch kit, and release 1.0.0 and launch.

## Out of scope

Code signing, a Mac app, translating the interface, Pro checkout, telemetry, and rewriting git history.
