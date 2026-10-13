# Uninstaller keeps the Library, optionally deletes app data

Status: ready-for-human
Blocked by: 01
Spec: ../spec.md

## What

Configure the NSIS uninstaller so it never touches `Documents\Meeting Minutes`, tells the user their Meetings are kept, and offers an unticked checkbox to also delete app data (models, settings, index, log).

## Acceptance criteria

- [x] Default uninstall leaves the Library and app data in place.
- [x] Ticking the box removes app data only; the Library is untouched.
- [x] The uninstaller text says where the Meetings are kept.
- [ ] Manual check on the laptop.

## Comments

- 2026-10-13: Configured per-user NSIS, customized its unticked app-data checkbox, and added a pre-uninstall notice naming `Documents\Meeting Minutes`. Reviewed the Tauri v2.12.2 NSIS template: only a checked box removes the bundle ID's app-data directories, and it never targets Documents. On a Windows laptop, run both uninstall choices and check that the Library survives each, while app data survives only the default choice.
