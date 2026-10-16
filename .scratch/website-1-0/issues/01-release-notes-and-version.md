# 01: Version 1.0.0 and release notes file

Status: ready-for-agent

Bump the three versions to 1.0.0 (keep Cargo.lock consistent). Add `RELEASE_NOTES/v1.0.0.md` (feature-grouped "First release": record both Sources, local Whisper plus Groq/Gemini with own keys, 11 languages with Auto-detect, cited Summary and Ask, Import, Library as plain files, experimental Managed key, no telemetry). Make `.github/workflows/release.yml` use that file as the draft Release body, then append its existing SHA-256 and provenance lines. `scripts/check-release-version.mjs` must still pass for tag v1.0.0.

- [ ] Versions agree; check-release-version passes for v1.0.0
- [ ] Workflow uses the notes file; fails clearly if missing
