# Website in `website/`

Status: ready-for-agent
Blocked by: 06
Spec: ../spec.md

## What

Static site in `website/` following `website/DESIGN.md` and the approved mockup, deployed by GitHub Actions to `meeting-minutes.tech`. Pages: Home (what it does, "Free Beta · Windows 11" status, download button for the latest published Release, SmartScreen note, dated roadmap), Support, Changelog (from Releases), `llms.txt`. Privacy lands in ticket 14. Maker and contact in the footer.

## Acceptance criteria

- [ ] Download button and version come from the latest published Release; with none published, the page says "Coming December 2026" instead.
- [ ] Roadmap uses month + year; future months marked "target".
- [ ] Meets the acceptance criteria in `website/DESIGN.md`.
- [ ] Human step: owner points the `meeting-minutes.tech` DNS at the host.
