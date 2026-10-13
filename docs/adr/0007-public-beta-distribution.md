# Public Free Beta ships unsigned, with the Tauri updater on GitHub Releases

The first public release is a Windows 11 x64 Free Beta, version `0.1.0`. It ships as an **unsigned** per-user NSIS installer from GitHub Releases on the public repo, and the download page explains the SmartScreen warning. Code signing (Azure Artifact Signing or an OV certificate) is deferred to keep the free beta free of signing cost and identity validation, and comes before the app leaves Beta. The Tauri updater is in the first build: its private key is a GitHub Actions secret with a backup in the owner's password manager. A `vX.Y.Z` tag builds a **draft** Release with `latest.json`, and the owner publishes it by hand. The bundle identifier changes from `dev.ducdv.meetingminutes` to `app.meeting-minutes.tech`, matching the public domain, before anyone outside installs the app.

## Considered Options

- **OV certificate**: about $200–400 a year plus a hardware token, and SmartScreen reputation still has to build, so not worth it for a free beta.
- **Notify-only updates or no updates**: simpler, but a build without the updater can never update itself, so beta testers would be stuck on old bugs.
- **Private repo**: GitHub Releases on a private repo cannot serve public downloads or the update manifest. The repo stays public with no license (all rights reserved).

## Consequences

- Losing the updater private key means every installed copy can never auto-update again.
- Changing the identifier after release strands installed copies: they keep the old app-data folder (settings, Whisper models) and the old updater. The Library in `Documents\Meeting Minutes` does not depend on the identifier.
- Uninstalling never removes the Library. Deleting app data (models, settings) is an unticked option.
- No telemetry: diagnostics are a local log the user copies into a bug report themselves.
