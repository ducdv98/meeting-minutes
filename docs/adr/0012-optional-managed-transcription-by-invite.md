# Optional Managed transcription by invite, tried before the user's own keys

ADR 0011 said the app talks to no backend and transcription goes to the cloud only on the user's own keys. We now add **Managed transcription**: an experimental Meeting Minutes service that the user connects with a one-time Invite code from the operator. When connected, it is the first engine in the chain (Managed, then the user's own Groq key, then Gemini, then local Whisper). It is opt-in and by invite, so the app still works fully with no service: someone without an Invite code sees no change. This amends ADR 0011. How the service gets its transcription is the operator's concern and is out of scope for the app.

- Connecting counts as consent to cloud transcription, the same as saving a Groq key (ADR 0011). The service's privacy statement is shown next to the Invite code field.
- The app stores only the service's client credential (in the Credential Manager), never a provider key.
- The service accepts at most 180 s per upload, has no Vocabulary prompt, and does not report the detected language. So the app cuts managed uploads at pauses to 170 s or less, uses another engine to detect the Meeting language and sends it explicitly, and relies on Correction for names.
- Any failure after a few retries, including revocation, falls through to the rest of the chain. The service is never a single point of failure.

## Consequences

- Settings gains a Managed transcription section (experimental, by invite). The public site is not changed while the feature is experimental.
- Removing the feature later means removing that Settings section and one engine. Meetings already transcribed keep the "Managed transcription" engine name in their Transcript.
