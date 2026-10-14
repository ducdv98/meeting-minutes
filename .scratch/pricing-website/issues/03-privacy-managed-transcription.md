# 03: Privacy page covers the current Managed transcription flow and planned Accounts

**What to build:** A reader of the privacy policy learns accurately what happens today when they connect experimental Managed transcription with an Invite code, and what will change with Accounts, clearly separated. Current behaviour must be written from the app code: the Invite code connects one install; Managed transcription sends audio to the Meeting Minutes service, which forwards it to Groq; audio is deleted after processing, while each part's transcribed text may be kept for up to five minutes for retries; plus whatever the app stores locally for the Invite connection. The current statement that the app has "no account, server of its own" is corrected. Account registration, verified-email storage and usage counters are described in a section clearly labelled as planned, not current behaviour. See `.scratch/pricing-website/spec.md` (Current and future availability), `CONTEXT.md` and ADR 0014.

**Blocked by:** None (can start immediately)

**Status:** done

- [ ] The policy describes the current Invite-code Managed transcription flow (service → Groq), audio deletion after processing, and the up-to-five-minute retry cache of transcribed text
- [ ] The service destination and anything stored locally for the Invite connection match the app code
- [ ] No sentence claims the app has no server of its own while Managed transcription exists
- [ ] Account registration and verified-email storage appear only as clearly labelled planned behaviour
- [ ] Privacy tests are extended to cover the service destination and the retry-cache statement, and all website tests pass
