# 06: Remove "Meetless" from the working tree

**What to build:** Rewrite ADR 0001 neutrally (an own product, with no reference product named), and remove every "Meetless" mention from the other ADRs, `docs/research/`, `.scratch/` and `CONTEXT.md`. Git history is not rewritten. See `.scratch/v1-launch/spec.md`.

**Blocked by:** None

Status: ready-for-agent

- [ ] `grep -ri meetless` finds nothing outside `.git/` and `node_modules/`
- [ ] The ADRs still read coherently
