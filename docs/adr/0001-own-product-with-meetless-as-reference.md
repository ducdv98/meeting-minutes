# Own product, with Meetless as a product reference only

We build our own Windows meeting recorder from scratch and use [Meetless](https://github.com/hoangnb24/meetless) only as a reference for product behaviour (the record → transcribe → cited Ask loop, crash-safe Chunks, segment-ID Citations). We do not fork or copy its code: the repository has no LICENSE file (so it is all-rights-reserved by default), it depends on an AGPL-3.0 Paseo fork, and much of it is macOS / Mac App Store machinery we don't need.

## Consequences

- Borrow ideas and invariants, never source files, unless the Meetless author grants a license.
- Meetless's Electron + Expo + Paseo + Convex stack is not a constraint on our choices (see ADR-0003).
