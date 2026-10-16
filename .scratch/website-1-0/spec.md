# Website for the 1.0.0 release

Status: ready-for-agent

Goal: when v1.0.0 is published, meeting-minutes.tech offers a direct Windows download with a visible SHA-256, reads professionally, shows a 1.0.0 Changelog, and has current screenshots and copy.

Decisions (grilled with the owner):
- Bump the app to 1.0.0 (package.json, src-tauri/tauri.conf.json, src-tauri/Cargo.toml), tag v1.0.0 later. The owner publishes the Release; nothing is tagged, pushed or published by the agent.
- Download button links straight to the NSIS .exe of the latest published Release. Under it: version, size, "Windows 11, 64-bit", the SHA-256 (read at build time from the Release's `.sha256` asset), and the existing unsigned/SmartScreen note.
- A published-Release build with no readable SHA-256 fails rather than ship a download without it.
- Support page gets a "Verify your download" section: `Get-FileHash .\<installer> -Algorithm SHA256` and `gh attestation verify <installer> --repo ducdv98/meeting-minutes`.
- Remove "Windows app is coming" text and the text-only/no-screenshot allowance once a Release exists.
- Pro card: "Planned", no date, no "Coming soon".
- Footer: "Meeting Minutes. A local-first meeting recorder for Windows." and "© 2026 Meeting Minutes · Duc Dao". Privacy and Support: "Contact: admin@meeting-minutes.tech", no "Email Duc Dao". Reword llms.txt the same way.
- Changelog: remove the hard-coded "Cloud transcription · In development" entry; 1.0.0 comes from the GitHub Release body, which comes from a repo file `RELEASE_NOTES/v1.0.0.md` that release.yml uses as the draft body (followed by the SHA-256 lines it already adds).
- Re-run scripts/screenshots and replace all 12 website screenshots.
- Copy covers: skippable first-run setup, 11 languages with Auto-detect, download/delete local models in Settings, choosing a Language model per Ask or Summary, re-transcribing with one chosen model, Managed key, Privacy tab showing where data can leave the PC. No accuracy/speed claims. No Accounts. Use CONTEXT.md terms.

Out of scope: signing the installer, tagging, publishing, pushing.
