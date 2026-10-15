# 14: Verifiable unsigned releases

**What to build:** Add `actions/attest-build-provenance` and a SHA-256 checksum asset to `release.yml`. The release notes template links a VirusTotal scan. The support page explains why the installer is unsigned (free, open source) and how to verify it (`gh attestation verify`, the checksum) or build it yourself. See `.scratch/v1-launch/spec.md`.

**Blocked by:** None

Status: ready-for-agent

- [ ] Draft Releases include `.sha256` and an attestation
- [ ] The support page has the verify and build-it-yourself instructions
- [ ] The SmartScreen guidance stays accurate
