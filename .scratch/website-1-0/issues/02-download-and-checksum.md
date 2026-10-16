# 02: Direct download with SHA-256

Status: ready-for-agent

See spec. Change website/build.mjs and website/lib/release.mjs: read the `.sha256` asset of the chosen Release (support `--fixture` with a checksum fixture for tests), show it in the hero download block, fail a published build without it, remove the "coming" states, add Verify section on support.html. Add tests.

- [ ] Hero shows button, version, size, Windows 11 64-bit, SHA-256, SmartScreen note
- [ ] Build fails without a checksum when a Release exists
- [ ] Support has "Verify your download"
- [ ] `node --test website/test/` passes
