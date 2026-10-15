# 03: First-run setup flow

**What to build:** On first launch, a skippable three-step flow opens: the Recording notice, then the model, then the Library location. The model step recommends a model for the PC's hardware (turbo when a GPU is present or the chosen language isn't English), shows its size, and offers **Download now** or **I'll use my own key** (which opens the Keys tab). Recording is allowed while the download runs. The flow can be reopened from Help. See `.scratch/v1-launch/spec.md`.

**Blocked by:** 02

Status: ready-for-agent

- [ ] The flow opens automatically only on first launch, and each step can be skipped
- [ ] The recommendation depends on hardware and language
- [ ] Download now starts the download from ticket 02 and shows its progress
- [ ] Help can reopen the flow
- [ ] Tests cover skip, download and use-my-key paths
