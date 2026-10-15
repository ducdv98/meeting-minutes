# 03: First-run setup flow

**What to build:** On first launch, a skippable three-step flow opens: the Recording notice, then the model, then the Library location. The model step recommends a model for the PC's hardware (turbo when a GPU is present or the chosen language isn't English), shows its size, and offers **Download now** or **I'll use my own key** (which opens the Keys tab). Recording is allowed while the download runs. The flow can be reopened from Help. See `.scratch/v1-launch/spec.md`.

**Blocked by:** 02

Status: ready-for-agent

- [x] The flow opens automatically only on first launch, and each step can be skipped
- [x] The recommendation depends on hardware and language
- [x] Download now starts the download from ticket 02 and shows its progress
- [x] Help can reopen the flow
- [x] Tests cover skip, download and use-my-key paths
- [x] Model step saves the default Meeting language and updates the recommendation
- [x] Library step opens the resolved folder; General shows that same path
- [x] `first_run_completed` is required in TypeScript Settings
- [x] Setup and Settings share model-download event reconciliation
- [x] First-run Library location comes from the initialised Library state
