# Tauri updater and tag-to-draft-Release workflow

Status: ready-for-human
Blocked by: 01
Spec: ../spec.md

## What

Add the Tauri updater plugin with the public key in config and the endpoint pointing at `latest.json` on the latest GitHub Release. On launch the app checks for updates and, if one exists, asks before downloading and installing. Add a GitHub Actions workflow: pushing a `vX.Y.Z` tag builds the NSIS installer on `windows-latest`, signs the update with `TAURI_SIGNING_PRIVATE_KEY` from secrets, and creates a **draft** Release with the installer, signature and `latest.json`. Fails if the tag and app version differ.

## Acceptance criteria

- [ ] Workflow produces a draft Release with installer, `.sig` and `latest.json`; nothing is published automatically.
- [x] Tag/version mismatch fails the workflow.
- [ ] App with no network or no Release starts normally and logs the failed check quietly.
- [ ] Update prompt never interrupts an active Recording; installing waits until no Recording runs.
- [ ] Manual (ticket 04): install `v0.1.0-test`, publish a higher test Release, and the app updates itself.

## Comments

- 2026-10-13: Added the pinned Tauri updater, launch check, native prompt, recorder idle guard, fixed diagnostics events, and a tag-to-draft-Release workflow. The tag/version guard passed local positive and mismatch tests; core, frontend typecheck, frontend tests, and build passed. The owner still needs to replace the ticket 04 public-key placeholder, set both signing secrets, review and publish a draft Release, and complete the Windows update round trip. The Release and Windows runtime criteria remain unticked until those checks run.
- 2026-10-13: Addressed review findings: the Release workflow passes the tag through an environment variable, rejects an existing published Release before building, and avoids persisted checkout credentials; Linux CI runs the tag/version tests. Update prompts and installs now wait for recording, saves, queued or running processing, imports, and AI jobs to finish. A shared start gate rejects new recording, save retries, imports, processing, Summary, and Ask work while an update prompt or installation is active. Core tests, typecheck, frontend tests, tag/version tests, and Windows-target clippy (with the whisper stub) passed. Windows runtime and Release asset criteria remain open.

### Releasing

Replace the public-key placeholder with the ticket 04 key and set `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` as repository secrets. Push a matching `vX.Y.Z` tag → the workflow creates a draft Release with the NSIS installer, signature, and `latest.json` → review the assets → Publish the Release manually.
