# 01: Remove "Beta" from public site copy

**What to build:** A visitor sees no "Beta" label anywhere on the site, while the Windows release is still honestly presented as unavailable until a real published installer exists, with December 2026 clearly marked as a target. Covers the header badge (and its styling), home hero, closing action, roadmap row and FAQ answers ("Is it free?", the unsigned-installer answer), the support page including the SmartScreen note, the privacy page's "Applies to" line, the Changelog fallback text, the generated `llms.txt`, and the website design documentation. The factual unsigned-installer / SmartScreen guidance stays, reworded where it refers to "leaving Beta". See `.scratch/pricing-website/spec.md` (Public presentation) and `CONTEXT.md`.

**Blocked by:** None (can start immediately)

**Status:** done

- [ ] No "Beta" (case-insensitive) appears in site-authored copy: header, home, support, privacy, Changelog fallback, generated `llms.txt`, and website design documentation
- [ ] With no published Release, the site shows no download button and shows the Windows release as December 2026 (target)
- [ ] With a published Release (fixture), release-derived download behaviour is unchanged: real installer link, version, size, and the unsigned / SmartScreen link
- [ ] SmartScreen guidance on the support page remains accurate and no longer references Beta
- [ ] A test builds the site with no Release and with a fixture Release and fails if any built page or `llms.txt` contains "Beta"
- [ ] Existing website tests pass
