# Redesign Settings and add Help/About

Status: ready-for-human
Blocked by: 01, 02, 07
Spec: ../spec.md

## What

Rebuild Settings (models, Meeting language, Vocabulary, AI providers, auto-Processing) to the mockup. Add Help/About: version with Beta badge, keyboard shortcuts, Copy diagnostics, reopen first-run guide, support email, privacy link, maker.

## Acceptance criteria

- [ ] Matches the mockup in light and dark.
- [x] Copy diagnostics puts the ticket 02 text on the clipboard and confirms.
- [x] Version shown comes from the app.

## Comments

- 2026-10-13: Codex hit its usage limit, so Claude implemented this ticket.
  - **Settings** is rebuilt to the mockup and saves each change straight away. It has:
    - Transcription model cards, with size and Downloaded or "Downloads when first used", via a new `installed_models` command.
    - A new Default language that Transcription uses when a Meeting has no language of its own. This is core `Settings.language` and `TranscribeRequest.default_language`, with tests.
    - The Library folder, Vocabulary (saved on blur and when leaving Settings), and the "Process after I stop" switch.
    - One provider card for Summary and Ask, with all the earlier AI key, model and test behaviour kept.
    - Saves run one at a time and a failed save offers Retry.
  - **Help** is a new sidebar item. It shows:
    - The wordmark, a Beta badge and the version from `app_info`.
    - The maker, plus support and privacy links that open through a new `open_link` command with a fixed list.
    - The real keyboard shortcuts.
    - A diagnostics preview and Copy diagnostics, which copies to the clipboard and confirms.
  - **Left out on purpose:**
    - "Start with Windows", because no such feature exists.
    - "up to date", because the page gets no updater status.
    - "Recommended for this PC": Small is tagged "Default" until ticket 12.
    - The Download link, because models download when first used.
    - Space play/pause, because there is no such shortcut.
    - "Open the setup guide" shows only when App passes `onOpenGuide`, which ticket 12 wires up.
  - **Verified:** `pnpm typecheck`, `pnpm test` (108 passed), `pnpm build`, `cargo test -p meeting_minutes_core` (137 passed), and the Windows check (`Finished`, no warnings).
  - **Laptop check still needed:**
    - Compare both screens with the mockup in light and dark.
    - Copy diagnostics into Notepad.
    - Confirm the privacy link opens the browser and the support link opens the mail app.
