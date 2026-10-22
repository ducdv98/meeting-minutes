# Contributing to Meeting Minutes

Thanks for helping. Meeting Minutes is open source under the [MIT license](LICENSE), and bug reports, fixes, docs and features are all welcome.

By contributing, you agree that your contribution is released under the same license.

## Before you start

- **Bugs and small fixes:** open a pull request directly.
- **Features or larger changes:** open a [GitHub issue](https://github.com/ducdv98/meeting-minutes/issues) first so we can agree on the approach before you spend time on it.
- **Security problems:** do not open a public issue. Email [admin@meeting-minutes.tech](mailto:admin@meeting-minutes.tech).
- **Not covered by the license:** the brand artwork in `assets/brand/` and files derived from it. Don't submit changes to them without asking first.
- **Managed transcription:** the Managed service is private and not part of this repository (ADR 0013). The client code for it is here and open to contributions.

## Understand the project

- `CONTEXT.md` is the domain vocabulary (Meeting, Recording, Transcript, Snapshot, Library and so on). Use these terms in code, UI text and docs.
- `docs/adr/` records the decisions already made, such as local-first AI with explicit cloud use (ADR 0002). Read the relevant ADRs before changing behaviour they cover. If your change contradicts one, say so in the pull request.
- Layout: see **Project structure** in the [README](README.md). The Rust core is in `crates/core`, Whisper bindings in `crates/whisper`, the Tauri app in `src-tauri`, the React UI in `src`, and the website in `website/` (see `website/README.md`).

## Set up

You can work on the Rust core and the React UI on Linux, macOS or Windows; running the full app and capture needs Windows 11.

### Prerequisites

- Windows 11: Rust stable with the MSVC toolchain and Microsoft C++ Build Tools, Node.js LTS, pnpm 12, Microsoft Edge WebView2 Runtime, and the Vulkan SDK (1.4.309.0, with `VULKAN_SDK` set) for GPU transcription. WebView2 is usually already installed on Windows 11.
- Linux: Rust, Node.js and pnpm for the core and UI tests. Running or building the Tauri app on Linux also needs the GTK/WebKit development libraries; the test commands below don't.

### Develop and build on Windows

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
powershell -File scripts/copy-vulkan-loader.ps1
pnpm tauri build --bundles nsis --config src-tauri/tauri.vulkan.conf.json
```

The installer is written to `target/release/bundle/nsis/`. The Vulkan build of whisper.cpp fails on paths longer than 260 characters, so clone to a short path or set `CARGO_TARGET_DIR` to one (CI uses `D:\t`). The extra config bundles the Vulkan loader (`vulkan-1.dll`) next to the exe, so the app also starts on PCs without a GPU driver and transcribes on the CPU.

Run `pnpm screenshots` to regenerate the demo screenshots used in the README and website.

## Make a change

1. Fork the repository and branch from `main`.
2. Keep the change focused: one fix or feature per pull request, with no unrelated reformatting.
3. Match the style of the surrounding code, including naming and comment density.
4. Add or update tests for behaviour you change. Core logic belongs in `crates/core` with Rust tests; UI behaviour is tested with Vitest and Testing Library.
5. Run the checks CI runs:

   ```sh
   cargo test -p meeting_minutes_core
   cargo test -p capture-spike
   node --test scripts/check-release-version.node.mjs
   pnpm typecheck
   pnpm test
   pnpm build
   ```

   On Windows, also run `cargo test --workspace` after `pnpm build`.
6. Update docs when behaviour changes: the README, the website, and an ADR in `docs/adr/` if you made a decision that someone would otherwise have to rediscover.

## Privacy rules

Meeting Minutes is local-first and has no telemetry. Please keep it that way:

- Don't add analytics, tracking or any network call that isn't an explicit, user-initiated feature.
- Anything that sends audio or Transcript text off the PC must be opt-in, under the user's own key or Managed key, and described in the privacy text.
- Never log Transcripts, Summaries, audio or keys. Diagnostics must not contain them either. Keys belong in Windows Credential Manager.

## Pull requests

- Describe what changed and why, and link the issue.
- Include screenshots or a short clip for UI changes.
- Say how you tested it, and on which OS. If you could not test on Windows, say so.
- CI must pass. Maintainers may ask for changes before merging.

## Reporting bugs

Use the bug report template. In the app, open **Help → Copy diagnostics** and paste the text, after reviewing it. Don't paste Transcripts or keys.

## Releases

Maintainers cut releases (see [docs/releasing.md](docs/releasing.md)). Contributors don't need to bump versions.
