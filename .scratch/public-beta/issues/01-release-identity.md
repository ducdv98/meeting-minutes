# Release identity: identifier, version and Beta badge

Status: ready-for-agent
Blocked by: none
Spec: ../spec.md

## What

Change the bundle identifier to `app.meeting-minutes.tech`, set version `0.1.0` in one place that the UI can read, and expose the version and "Beta" channel to the frontend for the About screen. No migration from `dev.ducdv.meetingminutes`.

## Acceptance criteria

- [ ] `tauri.conf.json` identifier is `app.meeting-minutes.tech`; app data lands in the matching folder.
- [ ] Version `0.1.0` is consistent across `tauri.conf.json`, `Cargo.toml` and `package.json`, and the UI reads it from the app, not a hard-coded string.
- [ ] The Library still loads from `Documents\Meeting Minutes` after the change.
- [ ] Credential Manager keys still resolve (they are stored under "Meeting Minutes", not the identifier).
