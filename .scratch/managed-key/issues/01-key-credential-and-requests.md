# Managed key credential and requests

Status: ready-for-agent
Spec: ../spec.md

## What
- Store the Managed key as one secret in the Credential Manager through the existing secret store; stop using `KeyringManagedStore`.
- Managed engine sends the key directly; remove the token manager's activate/refresh path.
- Map an invalid/revoked key response to a distinct error that the engine does not retry and that stops later runs using the key until it changes. Exhaustion and service pause still fall through.

## Acceptance criteria
- [ ] Core tests: key sent on each request; invalid key is not retried and is skipped on later runs; edited key clears the state; exhaustion falls through.
- [ ] Nothing about the key is written to the settings file.
- [ ] Windows clippy check on `src-tauri/src/main.rs` passes.
