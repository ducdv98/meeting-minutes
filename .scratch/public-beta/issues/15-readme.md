# Rewrite the README

Status: resolved
Blocked by: 03, 05
Spec: ../spec.md

## What

Replace the scaffold README: what the app is, download link, SmartScreen note, supported Windows, uninstall (Library is kept), privacy link, support; then developer sections for build, test, and releasing by tag. State that the code is not open source (no license, all rights reserved).

## Acceptance criteria

- [x] No mention of "scaffold" or CI-artifact installs for users.
- [x] Release steps match the ticket 03 workflow.

## Comments

- 2026-10-13: Codex was out of quota, so Claude rewrote `README.md`.
  - **For users:**
    - What the app does.
    - Download, with an honest "not published yet, planned December 2026" and links for when it is.
    - Windows 11 x64 support; Windows 10 may work but is untested.
    - The SmartScreen steps.
    - Uninstall: the Library is kept, the checkbox removes app data, and API keys stay in Credential Manager.
    - Privacy, support and Copy diagnostics, and the maker.
    - All rights reserved, with no license.
  - **For developers:** prerequisites, test, dev/build, the tag-to-draft-Release steps checked against `release.yml` and `check-release-version.mjs`, and branding.
  - **Review:** the Standards and Spec reviews found no breaches. Their suggestions (glossary terms, the API-key note, the capture-spike test, prerelease test tags) were applied.
