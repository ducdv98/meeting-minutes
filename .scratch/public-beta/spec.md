# Public Free Beta spec

Status: ready-for-agent

Turn the finished Windows MVP (`../windows-mvp/`) into a public Windows Free Beta, `0.1.0`, that a stranger can download, install, set up and trust. Vocabulary: `CONTEXT.md`. Distribution decisions: ADR-0007.

## Goal

Someone who finds `meeting-minutes.tech` downloads the beta, gets past SmartScreen, is guided through setup, records their first Meeting and gets a cited Summary, in an app that looks finished. Later builds reach them through the updater.

## Release gate

- Every ticket in `issues/` is resolved.
- A clean-machine test passes on a fresh Windows 11 x64 PC or VM with no Rust, Ollama or models installed: download from the website → SmartScreen → first-run guide → Record → Transcript → Summary → Ask → update to a newer test build.
- Target: release candidate by late November 2026, public `v0.1.0` in December 2026.

## Requirements

### Release identity and distribution (ADR-0007)
- Bundle identifier `app.meeting-minutes.tech`; product name "Meeting Minutes"; version `0.1.0`. The UI shows a "Beta" badge; the version number stays plain semver.
- Unsigned per-user NSIS installer. No migration from the old identifier (only the owner had it installed).
- Tauri updater from the first build: checks on launch, asks before installing, verifies the signature. Update manifest `latest.json` on GitHub Releases.
- Pushing a `vX.Y.Z` tag builds the installer, signs the update, and creates a **draft** GitHub Release with `latest.json` attached. The owner publishes by hand.
- Uninstall never removes the Library (`Documents\Meeting Minutes`) and says so. An unticked checkbox also deletes app data (downloaded models, settings, index).
- Officially supports Windows 11 x64. The installer does not block Windows 10; the website says "may work, untested".

### Diagnostics
- No telemetry and no network calls except model downloads, update checks and the AI provider the user chose.
- Rolling local log file in app data (size-capped). Never logs Transcript text, Summary or Ask content, or API keys.
- Help → Copy diagnostics: app version, Windows version, CPU/RAM, audio devices, selected model and provider kind, recent log lines. The user pastes it into an email or GitHub issue.

### App UI
- Visual direction: neutral modern, in the spirit of Linear and Notion. Calm, lots of whitespace, strong type hierarchy, mostly monochrome with one restrained accent (selection, links, Citations). Recording state is always red.
- Built on Tailwind with Radix-based (shadcn/ui-style) components. Light and dark themes follow Windows.
- Window remembers size and position; larger default than 900×650.
- Consistent loading, empty and error states across every panel.
- Help/About: version, Beta badge, keyboard shortcuts, Copy diagnostics, support contact, privacy link.
- An approved clickable mockup (ticket 06) is the visual spec for every UI ticket.

### First-run guide
- Shown on first launch; can be skipped and reopened from Help. Steps: Welcome → Recording notice → choose a Whisper model (size versus accuracy, recommended from this PC's hardware, download continues in the background) → Microphone and System audio level check → optional AI setup (Ollama or own key) → Library with an empty state ("Record or import a file").
- Replaces the separate first-Recording notice for users who complete it.

### Website (`website/` in this repo)
- Static site on `meeting-minutes.tech`, deployed by GitHub Actions. Same visual language as the app.
- Pages: Home (what it does, "Free Beta · Windows 11" status, download button for the latest Release, SmartScreen note, dated roadmap: Mac App Store Dec 2026, Windows Free Beta Dec 2026, later months marked "target"), Privacy, Support (`support@meeting-minutes.tech`, GitHub issues, how to Copy diagnostics), Changelog (from Releases), `llms.txt`. Maker (Duc Dao) named in the footer and About.
- Never overstate availability: no download button until a Release is published.
- The privacy policy is written from the code and states: everything stays on the PC; nothing is sent without a click; cloud Summary/Ask sends Transcript text to OpenAI or Anthropic under the user's own key; models download from Hugging Face; no telemetry.

### Docs
- README rewritten for both users (download, SmartScreen, uninstall) and developers (build, test, release by tag).

## Out of scope (beta)
Code signing (before leaving Beta), Microsoft Store, telemetry or crash reporting services, migration from the old identifier, Windows 10 support, Arm64, Mac app, billing, open-source license.
