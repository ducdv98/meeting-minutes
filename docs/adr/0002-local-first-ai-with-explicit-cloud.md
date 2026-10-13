# Local-first AI; cloud only on explicit request with the user's own key

> Amended by ADR 0011: with the user's own Groq key, cloud transcription and Correction run automatically after a one-time notice.

Transcription, Summary, Action items and Ask run on-device by default (Whisper via whisper.cpp; Ask via a local model through Ollama when it is installed). Cloud providers are optional, use the user's own API key (BYOK), and only run on an explicit user action. Local processing may start automatically after Stop; cloud processing never starts on its own. We chose this over Meetless's managed-cloud subscription because the first version is a personal tool. No backend means nothing to operate, and "your meetings never leave the PC unless you say so" is the privacy line we want to keep if this becomes a product.

## Considered Options

- **Managed cloud (Meetless's pivot)**: best quality with no setup, but needs a backend, billing, and per-hour provider cost from day one.
- **Reusing the user's Codex / Claude Code CLI login for Ask**: technically possible on Windows (no sandbox blocker like the Mac App Store one), but Anthropic's terms warn against routing third-party app traffic through Claude subscriptions, and CLI behaviour is not a stable product contract. OpenAI's "Sign in with ChatGPT" plan-usage route is a possible later option for open-source, locally hosted builds, and covers Ask only.

## Consequences

- Every engine writes the same provider-neutral Transcript format; app code assigns Segment IDs and checks every Citation.
- Quality and speed depend on the user's hardware; the app must benchmark the hardware and pick a model to match.
