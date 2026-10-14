# Gateway auth client and token manager

Status: ready-for-agent
Spec: ../spec.md

## What

- A core module for the Managed transcription service's auth, over the existing `HttpClient`:
  - `activate(invite_code)` → `client_id`, `refresh_token`, access token and expiry;
  - `refresh(refresh_token)` → access token and expiry.
- Service base URL: a constant `https://transcribe.meeting-minutes.tech`, overridden by `MEETING_MINUTES_GATEWAY_URL`.
- Parse the service's error envelope (`error.code`, `error.retryable`, `error.retry_after_ms`) into a typed error. Branch on the code, never on the message. A non-JSON reply counts as retryable.
- A token manager shared through an `Arc`:
  - built lazily from the stored refresh token;
  - holds the access token in memory;
  - refreshes 60 s before expiry;
  - offers "refresh now", used after a `401` on an upload;
  - when refresh returns `401`, reports Disconnected and clears the stored credentials through the secret store.
- `client_id` and `refresh_token` are stored through a small core trait for the Managed credential (get / set / delete), with an in-memory implementation for tests. `SecretStore` is keyed by `ProviderKind` and stays unchanged; the Credential Manager implementation over `keyring` lives in `src-tauri` and comes with ticket 02.
- Tokens and invite codes never appear in logs or `Debug` output.

## Acceptance criteria

- [ ] Core tests with a fake `HttpClient`:
  - activate success;
  - activate `401`;
  - refresh 60 s before expiry and not earlier;
  - "refresh now";
  - refresh `401` clears credentials and reports Disconnected;
  - error envelope parsed;
  - non-JSON reply treated as retryable;
  - env var override.
- [ ] No token or invite code is reachable through `Debug`/`Display`.
