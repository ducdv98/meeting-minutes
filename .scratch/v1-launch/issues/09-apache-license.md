# 09: Apply the Apache-2.0 license

**What to build:** Check the licenses of Rust and npm dependencies and bundled assets (whisper.cpp, models, fonts, brand images) for compatibility. Add `LICENSE` (Apache-2.0) and a NOTICE if needed, set the license fields in `Cargo.toml` and `package.json`, and lift the website's ban on saying "open source". See `.scratch/v1-launch/spec.md`.

**Blocked by:** None

Status: ready-for-human

- [x] The dependency check is recorded in `docs/research/`
- [ ] LICENSE is present and the manifests declare Apache-2.0
- [ ] Site copy may say "open source" and links to the repo
