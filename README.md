<img src="assets/brand/wordmark-dark.png" alt="Meeting Minutes" width="360">

# Meeting Minutes

Meeting Minutes records your calls on a Windows PC and turns them into Meetings in a personal Library. It captures your Microphone and your PC's System audio, so it works with any calling app, and transcribes them locally with Whisper by default. With your own Groq key, it uses `whisper-large-v3`; Gemini can handle fallback transcription. While a Recording runs, you can take Snapshots of a region of your screen, such as a shared slide; they appear in the Transcript and stay on your PC. You can then get a Summary with Action items and ask questions about one Meeting; answers cite the parts of the Transcript they come from, and say so when the Transcript doesn't answer the question. It is free.

Your Library stays on your PC. With your own keys, audio goes to Groq for transcription, or to Gemini for fallback transcription; Gemini also receives Transcript text for Correction. Summary, Action items and Ask can use Ollama locally or Gemini, OpenAI or Anthropic under your own key; the app asks before sending a Meeting. Gemini's free tier may use content for training. Keys stay in Windows Credential Manager. The app's only server is the optional Managed transcription service, which receives audio only after you paste in a Managed key; it never holds your Library.

## Download

Download it from [meeting-minutes.tech](https://meeting-minutes.tech) or from the [GitHub Releases page](https://github.com/ducdv98/meeting-minutes/releases/latest). The app checks for updates when it starts and asks before installing one.

- **Supported:** Windows 11 x64. Windows 10 may work but is untested.
- **SmartScreen:** the installer is not code-signed, so Windows may say it protected your PC. Download it only from the links above, then choose **More info → Run anyway**. To check the download first, see [Verify the installer](https://meeting-minutes.tech/support/#smartscreen).

## Uninstall

Use **Settings → Apps → Installed apps** in Windows. Your Library in `Documents\Meeting Minutes` is always kept. Tick the box in the uninstaller if you also want to remove the app's data (downloaded models, settings, search index and log). API keys stay in Windows Credential Manager after uninstalling, so remove them in the app's **Settings** first.

## Privacy and support

- Privacy policy: [meeting-minutes.tech/privacy](https://meeting-minutes.tech/privacy). There is no telemetry.
- Support: email [admin@meeting-minutes.tech](mailto:admin@meeting-minutes.tech) or open a [GitHub issue](https://github.com/ducdv98/meeting-minutes/issues). To include diagnostics, open **Help** in the app, choose **Copy diagnostics** and paste the text into your message. Diagnostics never include your Transcripts, Summaries or keys.

## Build from source

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

Maintainer guides: [releasing](docs/releasing.md) and [branding](docs/branding.md).

## Contributing

Contributions are welcome: bug reports, fixes, docs and features. Read [CONTRIBUTING.md](CONTRIBUTING.md) first, and open an [issue](https://github.com/ducdv98/meeting-minutes/issues) before starting anything large. The [Build from source](#build-from-source) section above covers building and testing.

## License

The app code is open source under the [MIT license](LICENSE). The brand artwork in `assets/brand/` and files derived from it are not covered by that license; all rights in them are reserved. The optional Managed transcription service is private and not part of this repository.

Made by Duc Dao and contributors with <3.
