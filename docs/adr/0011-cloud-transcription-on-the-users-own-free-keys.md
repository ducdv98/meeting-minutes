# Cloud transcription and Correction on the user's own free Groq and Gemini keys

Local Whisper (turbo `q5_0`) was neither accurate nor fast enough on many laptops, and we have at most $10 a month for 1,000 users, so an app-paid relay is ruled out (ADR 0008). Instead each user brings their own free API keys: Groq for transcription (Whisper large-v3, about 8 audio hours a day free) and Gemini for Correction, Summary, Action items and Ask. The app spends nothing, needs no backend, and each user's allowance comes from the provider directly. For now privacy is not a constraint: when a Groq key is set, transcription runs in the cloud automatically. A notice in Settings says that audio goes to the provider, but cloud use does not wait for it to be acknowledged: an unacknowledged notice left users with a saved key on the local model without knowing why. This amends ADR 0002 (cloud Processing no longer needs an explicit action) and supersedes ADR 0008's "cloud transcription deferred".

- Engine order per Chunk: Groq, then Gemini when Groq's quota is spent or it fails, then local Whisper when there is no key, no quota or no network. A Meeting may mix engines in one Revision; a notice says when it fell back.
- Gemini transcribes Chunks of about 30 s, so its model-written timestamps are off by at most one Chunk. This is accepted for a fallback.
- With a Groq key and a network, Chunks are uploaded during the Recording on any PC. ADR 0010's discrete-GPU and AC-power rule applies only to local transcription.
- Correction runs inside Processing, before the Revision is published. If it fails or Gemini's quota is spent, the uncorrected text is published.

## Considered Options

- **App-paid relay on a shared provider key** (ADR 0008's option): about $0.03 per audio hour at best, so $10 covers only a few hundred hours a month across all users.
- **Many free accounts behind a hidden relay, rotating their keys**: rejected. It breaks the providers' terms, Groq limits are per organisation rather than per key, and one ban would break transcription for every user at once.
- **Reusing the user's Claude Code or Codex login**: neither takes audio, and Anthropic does not permit routing app requests through Free, Pro or Max plan credentials.
- **Gemini as the main transcription engine**: its free limits are small and change often, and its timestamps are weaker than Whisper's.

## Consequences

- Setup gains a "get a free key" step for Groq and Gemini; without keys the app works as before, locally.
- Gemini's free tier uses content for training, and EEA, Swiss and UK users must use paid Gemini services. Revisit when privacy matters again.
- Which engine wins on Vietnamese is decided by the benchmark; Gemini may become the Vietnamese engine if it clearly beats Groq there.
