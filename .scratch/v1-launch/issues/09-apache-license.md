# 09: Apply the Apache-2.0 license

**What to build:** Check the licenses of Rust and npm dependencies and bundled assets (whisper.cpp, models, fonts, brand images) for compatibility. Add `LICENSE` (Apache-2.0) and a NOTICE if needed, set the license fields in `Cargo.toml` and `package.json`, and lift the website's ban on saying "open source". See `.scratch/v1-launch/spec.md`.

**Blocked by:** None

Status: ready-for-human

- [x] The dependency check is recorded in `docs/research/`
- [ ] LICENSE is present and the manifests declare Apache-2.0

Note 2026-10-20: deferred until after the Product Hunt launch. Blockers in `docs/research/dependency-licenses.md`: LGPL LAME static link, brand artwork rights.
- [ ] Site copy may say "open source" and links to the repo
