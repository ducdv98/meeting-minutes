<img src="assets/brand/wordmark-dark.png" alt="Meeting Minutes" width="360">

# Meeting Minutes

Meeting Minutes records your calls on a Windows PC and turns them into Meetings in a personal Library. It captures your Microphone and your PC's System audio, so it works with any calling app, and transcribes them locally with Whisper by default. With your own Groq key, it uses `whisper-large-v3`; Gemini can handle fallback transcription. While a Recording runs, you can take Snapshots of a region of your screen, such as a shared slide; they appear in the Transcript and stay on your PC. You can then get a Summary with Action items and ask questions about one Meeting; answers cite the parts of the Transcript they come from, and say so when the Transcript doesn't answer the question. It is free.

Your Library stays on your PC. With your own keys, audio goes to Groq for transcription, or to Gemini for fallback transcription; Gemini also receives Transcript text for Correction. Summary, Action items and Ask can use Ollama locally or Gemini, OpenAI or Anthropic under your own key; the app asks before sending a Meeting. Gemini's free tier may use content for training. Keys stay in Windows Credential Manager. The app's only server is the optional Managed transcription service, which receives audio only after you paste in a Managed key; it never holds your Library.

## Download

The first release is not published yet. When it is out, download it from [meeting-minutes.tech](https://meeting-minutes.tech) or from the [GitHub Releases page](https://github.com/ducdv98/meeting-minutes/releases/latest). The app checks for updates when it starts and asks before installing one.

- **Supported:** Windows 11 x64. Windows 10 may work but is untested.
- **SmartScreen:** the installer is not code-signed, so Windows may say it protected your PC. Download it only from the links above, then choose **More info → Run anyway**. To check the download first, see [Verify the installer](https://meeting-minutes.tech/support/#smartscreen).

## Uninstall

Use **Settings → Apps → Installed apps** in Windows. Your Library in `Documents\Meeting Minutes` is always kept. Tick the box in the uninstaller if you also want to remove the app's data (downloaded models, settings, search index and log). API keys stay in Windows Credential Manager after uninstalling, so remove them in the app's **Settings** first.

## Privacy and support

- Privacy policy: [meeting-minutes.tech/privacy](https://meeting-minutes.tech/privacy). There is no telemetry.
- Support: email [admin@meeting-minutes.tech](mailto:admin@meeting-minutes.tech) or open a [GitHub issue](https://github.com/ducdv98/meeting-minutes/issues). To include diagnostics, open **Help** in the app, choose **Copy diagnostics** and paste the text into your message. Diagnostics never include your Transcripts, Summaries or keys.

Made by Duc Dao.

## License

This code is not open source. No license is granted: all rights reserved. The repository is public so that GitHub Releases can serve downloads and updates.

---

## Developers

Domain vocabulary is in `CONTEXT.md` and decisions are in `docs/adr/`. The Rust core is in `crates/core`, Whisper bindings in `crates/whisper`, the Tauri app in `src-tauri`, the React UI in `src`, and the website in `website/` (see `website/README.md`).

### Prerequisites

- Windows 11: Rust stable with the MSVC toolchain and Microsoft C++ Build Tools, Node.js LTS, pnpm 12, Microsoft Edge WebView2 Runtime, and the Vulkan SDK (1.4.309.0, with `VULKAN_SDK` set) for GPU transcription. WebView2 is usually already installed on Windows 11.
- Linux: Rust, Node.js and pnpm for the core and UI tests. Running or building the Tauri app on Linux also needs the GTK/WebKit development libraries; the test commands below don't.

### Test

```sh
pnpm install --frozen-lockfile
cargo test -p meeting_minutes_core
cargo test -p capture-spike
node --test scripts/check-release-version.node.mjs
pnpm typecheck
pnpm test
pnpm build
```

On Windows, also run `cargo test --workspace` after `pnpm build` (the Tauri crate needs the built UI).

### Develop and build on Windows

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
powershell -File scripts/copy-vulkan-loader.ps1
pnpm tauri build --bundles nsis --config src-tauri/tauri.vulkan.conf.json
```

The installer is written to `target/release/bundle/nsis/`. The Vulkan build of whisper.cpp fails on paths longer than 260 characters, so clone to a short path or set `CARGO_TARGET_DIR` to one (CI uses `D:\t`). The extra config bundles the Vulkan loader (`vulkan-1.dll`) next to the exe, so the app also starts on PCs without a GPU driver and transcribes on the CPU.

### Release

Releases are built by `.github/workflows/release.yml` and published by hand (ADR-0007).

One-time setup: generate the updater key pair with `pnpm tauri signer generate`, back up the private key and its password outside GitHub (losing it means installed copies can never update again), add them as the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, and put the public key in `plugins.updater.pubkey` in `src-tauri/tauri.conf.json`.

For each release:

1. Set the same version in `src-tauri/tauri.conf.json`, `package.json` and `src-tauri/Cargo.toml`, and commit.
2. Tag the commit and push the tag:
   ```sh
   git tag v0.1.0
   git push origin v0.1.0
   ```
   A tag with a prerelease suffix, such as `v0.1.1-test`, builds a test Release the same way (the versions must say `0.1.1-test`).
3. The workflow checks that the tag matches all three versions and refuses a tag whose Release is already published. It runs the tests and builds the NSIS installer, then creates a **draft** GitHub Release with the installer, its `.sig` signature and `latest.json`.
4. Review the draft and its assets, then choose **Publish release**. Nothing is published automatically. Publishing also rebuilds the website, so its download link, version and Changelog follow the new Release.

### Branding

The brand pack in `assets/brand/` holds the original files and is never edited. The build uses files derived from it: `src-tauri/app-icon.png` and `src-tauri/icons/` for the app, and `src/assets/` for the UI. To rebuild them after changing the pack:

```sh
python scripts/build-brand-assets.py      # needs Pillow
pnpm tauri icon src-tauri/app-icon.png    # then keep only the files listed in tauri.conf.json
```

See `docs/adr/0004-branding-from-the-image-pack.md`.
