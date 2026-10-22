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
- Layout: Rust core in `crates/core`, Whisper bindings in `crates/whisper`, the Tauri app in `src-tauri`, the React UI in `src`, the website in `website/`.

## Set up

See **Build from source** in the [README](README.md) for prerequisites. In short:

- You can work on the Rust core and the React UI on Linux, macOS or Windows; running the full app and capture needs Windows 11.
- Install dependencies with `pnpm install --frozen-lockfile`.
- Run the app on Windows with `pnpm tauri dev`.

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
