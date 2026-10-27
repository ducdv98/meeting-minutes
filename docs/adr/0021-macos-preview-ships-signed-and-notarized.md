# macOS Preview ships signed and notarized

> Departs from ADR 0007 for macOS only. Windows stays unsigned until it leaves Beta.

The Windows Beta ships unsigned to avoid signing cost (ADR 0007), but the macOS Preview is signed with a Developer ID certificate (Apple Developer Program, $99 a year) and notarized in CI from its first build. On macOS an unsigned app is far worse than a SmartScreen warning: since macOS 15 the user has to approve it in System Settings, and with ad-hoc signing macOS can reset the Microphone and System Audio Recording permissions after every update. A recorder that silently loses its permissions after each update cannot be trusted.

## Considered Options

- **Unsigned or ad-hoc signed, like Windows**: free, but every update risks losing the Microphone and System audio permissions, and installing it needs a trip to System Settings.

## Consequences

- The Developer ID certificate, the notarization credentials and the updater key are GitHub Actions secrets, each with a backup in the owner's password manager.
- The app bundle declares Microphone, System Audio Recording and Documents usage strings, plus the hardened-runtime entitlements Whisper and audio capture need.
- An expired Apple Developer membership stops new macOS releases, but already-installed copies keep working.
