# Release identity: identifier, version and Beta badge

Status: ready-for-human
Blocked by: none
Spec: ../spec.md

## What

Change the bundle identifier to `app.meeting-minutes.tech`, set version `0.1.0` in one place that the UI can read, and expose the version and "Beta" channel to the frontend for the About screen. No migration from `dev.ducdv.meetingminutes`.

## Acceptance criteria

- [x] `tauri.conf.json` identifier is `app.meeting-minutes.tech`; app data lands in the matching folder.
- [x] Version `0.1.0` is consistent across `tauri.conf.json`, `Cargo.toml` and `package.json`, and the UI reads it from the app, not a hard-coded string.
- [ ] The Library still loads from `Documents\Meeting Minutes` after the change.
- [x] Credential Manager keys still resolve (they are stored under "Meeting Minutes", not the identifier).

## Comments

- 2026-10-13: Changed the bundle identifier and added `app_info` with a typed frontend wrapper. All version declarations remain `0.1.0`; the app reports its package version. Code inspection confirms the Library uses Documents and Credential Manager still uses the `Meeting Minutes` service. On a Windows laptop, verify the new app-data directory, existing Library loading, and saved keys after install. The `src/app-info.ts` UI wrapper is wired into the About screen in ticket 11; that criterion remains unticked until then.
