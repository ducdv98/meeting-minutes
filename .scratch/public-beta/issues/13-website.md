# Website in `website/`

Status: ready-for-human
Blocked by: 06
Spec: ../spec.md

## What

Static site in `website/` following `website/DESIGN.md` and the approved mockup, deployed by GitHub Actions to `meeting-minutes.tech`. Pages: Home (what it does, "Free Beta · Windows 11" status, download button for the latest published Release, SmartScreen note, dated roadmap), Support, Changelog (from Releases), `llms.txt`. Privacy lands in ticket 14. Maker and contact in the footer.

## Acceptance criteria

- [x] Download button and version come from the newest published Release with a Windows installer; with none published, the page says "Free Beta for Windows: December 2026 (target)" instead.
- [x] Roadmap uses month + year; future months marked "target".
- [ ] Meets the acceptance criteria in `website/DESIGN.md`.
- [ ] Human step: owner points the `meeting-minutes.tech` DNS at the host.

## Comments

### 2026-10-13

Built the static Home, Support, Changelog, `llms.txt`, 404 and draft Privacy pages in `website/`, with shared identity styling, self-hosted fonts, brand assets, release-driven download state, and a GitHub Pages workflow. The pure release tests pass. A draft offline build succeeds; a non-draft offline build correctly lists the privacy stub and all missing light/dark screenshots. A release fixture showed the download link, version, size and Changelog; the no-release build showed the December 2026 target without download links. Generated pages contain only the repository and site URLs.

The remaining design acceptance check needs ticket 14's final privacy policy, real app screenshots after the UI rebuild, and visual/keyboard checks in a browser at the specified widths. The owner must enable GitHub Pages with the GitHub Actions source, set the custom domain, point the apex DNS records to GitHub Pages, and enforce HTTPS after the certificate is ready. See `website/README.md` for the records and steps. No deployment was run.

Review findings addressed in the uncommitted website work: the workflow now runs tests and a draft offline build for changes, and its deploy job reports missing prerequisites in the job summary without failing main. Publishing requires the privacy policy and all six light/dark screenshot pairs, including Search, Copy diagnostics and the first-run model guide. Release parsing now rejects malformed dates and deceptive installer URLs, follows API pagination links, and falls back to the newest Release with an installer. A published Release drives the roadmap's Available date from the first publication month. The Ask copy, hit areas, FAQ and scroll motion, privacy panel and type scale were revised against the design notes. Ticket 16 now calls for a claim-by-claim check against the release build. Verification: nine website tests passed; the draft offline build passed; readiness reported thirteen missing items with exit 0; the non-draft offline build failed cleanly; a two-Release fixture showed “December 2026” and “Available” without “(target)”; mocked API checks followed 21 pages and reported the 429 rate-limit headers.
