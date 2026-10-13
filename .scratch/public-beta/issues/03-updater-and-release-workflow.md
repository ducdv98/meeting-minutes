# Tauri updater and tag-to-draft-Release workflow

Status: ready-for-agent
Blocked by: 01
Spec: ../spec.md

## What

Add the Tauri updater plugin with the public key in config and the endpoint pointing at `latest.json` on the latest GitHub Release. On launch the app checks for updates and, if one exists, asks before downloading and installing. Add a GitHub Actions workflow: pushing a `vX.Y.Z` tag builds the NSIS installer on `windows-latest`, signs the update with `TAURI_SIGNING_PRIVATE_KEY` from secrets, and creates a **draft** Release with the installer, signature and `latest.json`. Fails if the tag and app version differ.

## Acceptance criteria

- [ ] Workflow produces a draft Release with installer, `.sig` and `latest.json`; nothing is published automatically.
- [ ] Tag/version mismatch fails the workflow.
- [ ] App with no network or no Release starts normally and logs the failed check quietly.
- [ ] Update prompt never interrupts an active Recording; installing waits until no Recording runs.
- [ ] Manual (ticket 04): install `v0.1.0-test`, publish a higher test Release, and the app updates itself.
