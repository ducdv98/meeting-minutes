# Open-source app with a private Managed transcription service

Before public account-based Managed access launches, release the app code under a permissive open-source license while keeping the Managed transcription service private. Modified commercial builds may keep their changes private and connect to the service with their users' own Accounts; access is granted by the service to an Account, not by whether the client is an official build. Brand artwork is outside the app-code license until its reuse rights are confirmed. This replaces the all-rights-reserved distribution decision in ADR 0007 and the current README; the exact license and release work are still pending.

## Considered Options

- **Keep the app all rights reserved:** this is the current public-repository arrangement, but does not meet the decision to keep the app open source.
- **Publish the service too:** this would expose service internals without being needed to make the local app and its own-key integrations open source.

## Consequences

- Check the statically linked LAME dependency, bundled fonts and brand-pack rights, then select and apply a permissive open-source license before describing the app as open source publicly.
- Authenticate and limit Managed transcription by Account even when requests come from a modified client.
