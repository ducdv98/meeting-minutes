# 06: Remove the reference product's name from the working tree

**What to build:** Rewrite ADR 0001 neutrally (our own product, with no reference product named), and remove every mention of the reference product's name from the other ADRs, `docs/research/`, `.scratch/` and `CONTEXT.md`. Git history is not rewritten. See `.scratch/v1-launch/spec.md`.

**Blocked by:** None

Status: ready-for-agent

- [x] A case-insensitive search finds no reference product name outside `.git/` and `node_modules/`
- [x] The ADRs still read coherently
