# Meeting Minutes website

Static HTML and CSS for `meeting-minutes.tech`. `build.mjs` uses Node built-ins and the app's existing `@fontsource` packages; the browser loads only files in `website/dist/`.

## Local preview

From the repository root, with the existing dependencies present:

```sh
node --test website/test/
node website/build.mjs --draft --offline
```

Serve `website/dist/` with any static file server. The draft build uses neutral “Screenshot” frames where assets are missing. `--offline` gives an empty release list; `--fixture website/test/release-fixture.json` loads a GitHub Releases API array for local checks. A checksum asset in a fixture uses `fixture_path`, relative to the JSON file, or inline `content` for its `.sha256` contents. `--output path/to/directory` writes to another directory. Omit both release flags to fetch Releases from GitHub. `GITHUB_TOKEN` is used if set.

The Request a key page uses a pre-filled email draft. It has no form backend. The pricing page describes the Managed key allowance without promising a specific number of minutes during the experiment.

Run `node website/build.mjs --check-ready` to list what blocks publishing; it exits successfully and writes `ready=true` or `ready=false` to `GITHUB_OUTPUT` when set. The privacy page must be the real policy (no `PRIVACY_STUB_TICKET_14` marker). Once a Release is published, all twelve screenshots listed in [`src/assets/screens/README.md`](src/assets/screens/README.md) are required. A published Release needs a valid NSIS `.exe` and a matching readable `.sha256` asset. The build fails when either is missing or invalid; it does not expose an older installer.

## One-time owner setup

The site is served by a static-only Cloudflare Worker (`wrangler.jsonc`) on the `meeting-minutes.tech` zone, which is already on Cloudflare DNS.

1. In the Cloudflare dashboard, create an API token from the **Edit Cloudflare Workers** template, limited to your account and the `meeting-minutes.tech` zone. Add it to the repository secrets as `CLOUDFLARE_API_TOKEN`, and your account ID as `CLOUDFLARE_ACCOUNT_ID`.
2. Remove any existing record for the apex `meeting-minutes.tech` (a CNAME there blocks the Custom Domain). The first deploy creates the DNS record and certificate itself.
3. Keep **Web Analytics** and **Bot Fight Mode** off for the zone: the privacy policy promises no analytics, and Bot Fight Mode sets a `__cf_bm` cookie.
4. Run the **Website** workflow (or push to `main`). While screenshots are missing, the deploy job lists them in the run summary and skips deploying. After the first deploy, open https://meeting-minutes.tech/ and check HTTPS.

Publishing a Release triggers the website workflow so the download URL, version, size, SHA-256 and Changelog refresh from GitHub Releases. Do not deploy a `--draft` build.

The Changelog shows only published Release notes. The hero download links to the selected NSIS installer and displays its SHA-256, version, size and Windows requirement. Release notes are maintained in `RELEASE_NOTES/v1.0.0.md` and copied into the draft Release before the workflow appends verification details.
