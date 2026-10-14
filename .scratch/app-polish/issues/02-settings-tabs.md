# Settings tabs and regrouping

Status: ready-for-agent
Spec: ../spec.md

## What

- Split `SettingsView` into tabs using `components/ui/Tabs`: **General** · **Speech-to-text** · **Language models** · **Keys** · **Privacy**.
- Move existing controls, no behaviour change yet:
  - General: Your name, Default language, Library folder, Process after I stop.
  - Speech-to-text: Managed transcription, local Whisper model choice, Vocabulary, the Groq key block (moves to Keys in 03).
  - Language models: the current Summary and Ask provider section (reshaped in 05).
  - Keys and Privacy: placeholders filled by 03 and 07.
- Split the 260-line component into one file per tab under `src/settings/`.
- Tab change fades content (120ms). Remember the last tab for the session; opening Settings from a link (later tickets) can choose the tab.
- The "Saved" indicator stays in the page header and works on every tab.

## Acceptance criteria

- [ ] Every existing Settings control is reachable on the tab listed above and behaves as before.
- [ ] Existing `SettingsView.test.tsx` tests pass, updated only to switch tabs.
- [ ] Tabs work by keyboard (arrow keys, Tab into content).
- [ ] Settings can be opened on a given tab (used by 05, 11).
