# 02: Download local models from Settings

**What to build:** Settings → Speech-to-text gets a Download button with progress and Cancel for each local Whisper model, and Delete for downloaded ones. The VAD model downloads together with the first Whisper model. When Processing needs a model that isn't downloaded, it waits and shows "Waiting for model download (x%)" without failing. The words "Downloads when first used" go away. See `.scratch/v1-launch/spec.md`.

**Blocked by:** None

Status: ready-for-agent

- [x] Each model shows Download / progress % / Cancel / Delete
- [x] The VAD model is fetched with the first Whisper model, with no separate prompt
- [x] Processing shows "Waiting for model download (x%)" and continues when the download finishes
- [x] A failed or cancelled download leaves no partial model file
- [x] No UI text says "Downloads when first used"
- [x] Managed Auto-detect uses English when no local model download is active, and detects after an active download completes
- [x] Processing stops waiting when a model download fails, is cancelled, or was never started
- [x] Cancellation during finalization does not install the model file
