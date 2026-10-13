# Generate the updater key and set the GitHub secret

Status: ready-for-human
Blocked by: none
Spec: ../spec.md

## What

The owner generates the Tauri updater key pair (`pnpm tauri signer generate`), stores the private key and password in their password manager, adds them as `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` repository secrets, and gives the public key for ticket 03. Then runs the update round-trip test from ticket 03.

## Acceptance criteria

- [ ] Private key backed up outside GitHub (ADR-0007: losing it strands every install).
- [ ] Secrets set; public key committed in config.
- [ ] Update round trip passes on the laptop.
