# 02: Download local models from Settings

**What to build:** Settings → Speech-to-text gets a Download button with progress and Cancel for each local Whisper model, and Delete for downloaded ones. The VAD model downloads together with the first Whisper model. When Processing needs a model that isn't downloaded, it waits and shows "Waiting for model download (x%)" without failing. The words "Downloads when first used" go away. See `.scratch/v1-launch/spec.md`.

**Blocked by:** None

Status: ready-for-agent

- [ ] Each model shows Download / progress % / Cancel / Delete
- [ ] The VAD model is fetched with the first Whisper model, with no separate prompt
- [ ] Processing shows "Waiting for model download (x%)" and continues when the download finishes
- [ ] A failed or cancelled download leaves no partial model file
- [ ] No UI text says "Downloads when first used"
