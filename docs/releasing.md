# Releasing

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
