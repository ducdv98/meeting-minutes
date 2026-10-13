# Update public privacy copy

Status: ready-for-human
Spec: ../spec.md
Blocked by: 03

## What

- Update `website/src/privacy.html`, `website/src/home.html`, `README.md` and `website/README.md` where they say audio or transcripts never leave the PC. New message:
  - local by default;
  - with a Groq or Gemini key, audio and transcript text go to that provider under the user's own account;
  - Gemini's free tier may use content for training.
- Rebuild `website/dist` with the site's usual build.
- Add a changelog entry for cloud transcription.

## Acceptance criteria

- [ ] No public page still claims meetings never leave the PC without mentioning the cloud option.
- [ ] Site builds.
