# 07: Remove "Beta" from the app and docs

**What to build:** Drop the release channel from `app_info` and the About badge, so Help shows only the version. The diagnostics header becomes "Meeting Minutes <version>". Remove Beta from the README, `docs/design/visual-identity.md` (the Beta badge) and `docs/design/mockup.html`. Then remove the badge workaround from `scripts/screenshots/capture.mjs` and re-run `pnpm screenshots`. See `.scratch/v1-launch/spec.md`.

**Blocked by:** None

Status: ready-for-agent

- [ ] No user-visible "Beta" in the app, README or design docs
- [ ] The diagnostics test expects "Meeting Minutes 0.x.y" without Beta
- [ ] Screenshots are regenerated without the workaround
