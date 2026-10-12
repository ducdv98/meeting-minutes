# Meeting Minutes

A local-first Windows app for keeping Meetings in a personal Library. This scaffold currently shows an empty Library.

## Prerequisites

- Windows 11: Rust stable with the MSVC toolchain and Microsoft C++ Build Tools, Node.js LTS, pnpm 12, and Microsoft Edge WebView2 Runtime. WebView2 is usually already installed on Windows 11.
- Linux: Rust, Node.js, and pnpm for core and UI tests. Running or building the Tauri app on Linux also requires the GTK/WebKit development libraries; they are not needed for the commands below.

## Run tests

```sh
pnpm install --frozen-lockfile
cargo test -p meeting_minutes_core
pnpm typecheck
pnpm test
pnpm build
```

On Windows, also run `cargo test --workspace` (after `pnpm build`, which the Tauri crate needs) to test the Tauri crate.

## Develop and build on Windows

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
pnpm tauri build --bundles nsis
```

The installer is written to `target/release/bundle/nsis/`.

## Install a CI build

Open the repository's **Actions** tab, choose a successful CI run, and download **meeting-minutes-windows-installer** under **Artifacts**. Unzip it and run the NSIS `.exe` installer. The installer is unsigned; if Windows SmartScreen warns, choose **More info → Run anyway**.
