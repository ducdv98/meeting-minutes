# Uninstaller keeps the Library, optionally deletes app data

Status: ready-for-agent
Blocked by: 01
Spec: ../spec.md

## What

Configure the NSIS uninstaller so it never touches `Documents\Meeting Minutes`, tells the user their Meetings are kept, and offers an unticked checkbox to also delete app data (models, settings, index, log).

## Acceptance criteria

- [ ] Default uninstall leaves the Library and app data in place.
- [ ] Ticking the box removes app data only; the Library is untouched.
- [ ] The uninstaller text says where the Meetings are kept.
- [ ] Manual check on the laptop.
