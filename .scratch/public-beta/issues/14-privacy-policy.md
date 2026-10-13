# Privacy policy

Status: resolved
Blocked by: 02, 13
Spec: ../spec.md

## What

Write the privacy policy from the code, not from intentions: list every network destination the app can contact (Hugging Face model downloads, GitHub update check, Ollama on localhost, OpenAI/Anthropic on explicit click), what each receives, what is stored where (Library, app data, Credential Manager, log), no telemetry, and the recording consent responsibility. Publish it at `/privacy` and link it from the app's Help.

## Acceptance criteria

- [x] Every outbound call in the code is listed; a reviewer can trace each claim to code.
- [x] Dated; contact email included.

## Comments

### 2026-10-13 — Policy and evidence

- No app account, backend, ads, sale of data, or app telemetry → `docs/adr/0002-local-first-ai-with-explicit-cloud.md:1`; the runtime destinations are enumerated by `crates/core/src/ai/http.rs:66`, `crates/core/src/models.rs:44`, and `src-tauri/tauri.conf.json:50`; the log plugin filters to local event names in `src-tauri/src/main.rs:2026`. This is a code inventory claim, not a claim about Windows or provider diagnostics.
- Local transcription and possible automatic model download after Stop → `src-tauri/src/main.rs:1165`, `src-tauri/src/main.rs:1173`, `src-tauri/src/main.rs:1565`, `crates/core/src/models.rs:44`, `crates/core/src/models.rs:119`. The download is a model-file GET; it does not include Meeting content.
- GitHub update check at launch, prompt before package download, and signature verification → `src-tauri/tauri.conf.json:50`, `src-tauri/src/main.rs:1878`, `src-tauri/src/main.rs:1903`, `src-tauri/src/main.rs:1950`, `src-tauri/src/main.rs:2156`, `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tauri-plugin-updater-2.13.3/src/updater.rs:837`. The release workflow attaches the updater JSON at `.github/workflows/release.yml:51` and the website links the Release installer at `website/build.mjs:85`.
- Update request detail → `src-tauri/tauri.conf.json:50` has a static `latest.json` URL without version/platform placeholders; `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tauri-plugin-updater-2.13.3/src/updater.rs:568` only substitutes placeholders when present, `:590` sets `tauri-plugin-updater/2.13.3` as user agent, and `:669` compares the app version locally. The manifest's package URL is fetched at `:807`. IP address is ordinary HTTP connection metadata, not an app field.
- Ollama, OpenAI and Anthropic endpoints, model-list GETs and generation POST bodies → `crates/core/src/ai/http.rs:66`, `crates/core/src/ai/http.rs:109`, `crates/core/src/ai/http.rs:174`, `crates/core/src/ai/http.rs:258`. OpenAI sends the key in the Authorization header and Anthropic in `x-api-key` at `crates/core/src/ai/http.rs:116`.
- Model listing and fixed test message require Settings clicks → `src/SettingsView.tsx:74`, `src/SettingsView.tsx:80`, `src-tauri/src/main.rs:599`, `src-tauri/src/main.rs:655`. They send no Meeting text; the test prompt is at `src-tauri/src/main.rs:663`.
- Cloud Summary/Ask need a user action and first-use consent per Meeting/provider; later sends still need a click → `src/SummaryPanel.tsx:36`, `src/AskPanel.tsx:35`, `src-tauri/src/main.rs:715`, `crates/core/src/library.rs:450`, `src/CloudDisclosure.tsx:9`. The initial preview is assembled at `src-tauri/src/main.rs:764` and `src-tauri/src/main.rs:953`. No audio is in the AI request body at `crates/core/src/ai/http.rs:258`.
- Summary sends Transcript segments in chunks and can send generated partial summaries for merging → `crates/core/src/summary.rs:214`, `crates/core/src/summary.rs:225`, `crates/core/src/summary.rs:248`, `crates/core/src/summary.rs:410`. Ask sends a question, selected Transcript segments and sometimes prior questions/Answers → `crates/core/src/ask.rs:253`. Ollama Summary may run automatically after local transcription → `src-tauri/src/main.rs:1235`.
- Library path and contents → `src-tauri/src/main.rs:2059`, `crates/core/src/library.rs:165`, `crates/core/src/recording.rs:623`, `crates/core/src/recording.rs:649`, `crates/core/src/library.rs:216`, `crates/core/src/processing.rs:209`, `crates/core/src/summary.rs:69`, `crates/core/src/ask.rs:69`, `crates/core/src/library.rs:483`. The Library search index stores Transcript text and is rebuilt on a fresh index at `crates/core/src/index.rs:46` and `crates/core/src/library.rs:143`.
- Roaming app data paths and contents → `src-tauri/src/main.rs:2064`, `src-tauri/src/main.rs:2073`, `src-tauri/src/main.rs:2082`, `crates/core/src/settings.rs:11`, `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tauri-plugin-window-state-2.5.0/src/lib.rs:36`. Tauri resolves app data/config beneath the bundle identifier at `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tauri-2.12.3/src/path/desktop.rs:235` and `:247`.
- Local AppData/WebView2 and the capped event log → `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tauri-2.12.3/src/path/desktop.rs:259` and `:286`, `src-tauri/src/main.rs:2026`, `crates/core/src/diagnostics.rs:81`. The log allowlist and provider/status-only errors are at `crates/core/src/diagnostics.rs:114`, `:142`, and `:180`.
- API keys in Windows Credential Manager, with named entries and Settings removal → `src-tauri/src/main.rs:181`, `src-tauri/src/main.rs:203`, `src-tauri/src/main.rs:626`, `src/SettingsView.tsx:70`.
- Diagnostics are generated on request from versions, hardware, devices, provider and filtered recent log lines → `src-tauri/src/main.rs:77`, `src-tauri/src/main.rs:88`, `crates/core/src/diagnostics.rs:228`. The clipboard control is specified in `.scratch/public-beta/issues/11-settings-and-help.md:14` and belongs to ticket 11.
- WebView2 diagnostic behavior and Windows setting → Microsoft [WebView2 data and privacy](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/data-privacy); installer WebView2 download text is in `src-tauri/windows/English.nsh:21`.
- Website on GitHub Pages, local scripts/fonts and no website cookie or analytics code → `.github/workflows/website.yml:80`, `website/build.mjs:129`, `website/build.mjs:160`, `website/src/main.js:1`. GitHub documents Pages visitor IP logging in [What is GitHub Pages?](https://docs.github.com/en/pages/getting-started-with-github-pages/what-is-github-pages) and its [privacy statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement).
- Recording notice/indicator and user consent responsibility → `src/RecordingBar.tsx:105`, `src/RecordingBar.tsx:136`. Meeting deletion uses the Recycle Bin at `src-tauri/src/main.rs:1309` and `crates/core/src/recycle_bin.rs:3`; uninstall keeps the Library and offers app-data removal at `src-tauri/windows/hooks.nsh:1` and `src-tauri/windows/English.nsh:27`.
- Policy date, children statement, changes process and Duc Dao contact are policy commitments requested in this ticket; contact also appears at `website/build.mjs:137`.

`website/test/privacy.test.mjs` checks source/config URL hosts against the destination column and rejects the stub. It excludes test fixtures, Rust documentation comments, the config schema URL and the dev-only Vite URL because these are not release app connections. `node --test website/test/privacy.test.mjs` passes. `node --test website/test/` runs nine release tests through the existing `website/test/index.js`; it does not discover the new test file. That index file is outside this ticket's allowed edits.

The Help privacy link is owned by ticket 11 (`.scratch/public-beta/issues/11-settings-and-help.md:9`); no app UI file was changed here. `node website/build.mjs --draft --offline` succeeds, and `--check-ready` no longer lists Privacy; twelve screenshot assets remain as separate publish prerequisites.
