# Meeting Minutes website

Static HTML and CSS for `meeting-minutes.tech`. `build.mjs` uses Node built-ins and the app's existing `@fontsource` packages; the browser loads only files in `website/dist/`.

## Local preview

From the repository root, with the existing dependencies present:

```sh
node --test website/test/
node website/build.mjs --draft --offline
```

Serve `website/dist/` with any static file server. The draft build uses neutral “Screenshot” frames where assets are missing. `--offline` gives an empty release list; `--fixture path/to/releases.json` loads a GitHub Releases API array for local checks. Omit both flags to fetch Releases from GitHub. `GITHUB_TOKEN` is used if set.

Run `node website/build.mjs --check-ready` to list missing publish prerequisites; it exits successfully and writes `ready=true` or `ready=false` to `GITHUB_OUTPUT` when set. The non-draft build fails until ticket 14 replaces the `PRIVACY_STUB_TICKET_14` page marker and all twelve real app screenshots listed in [`src/assets/screens/README.md`](src/assets/screens/README.md) exist. A published Release is **not** required: with none, the site shows the December 2026 target and no download button. If the newest published Release has no NSIS `.exe` asset, the build uses the newest published Release that does.

## One-time owner setup

1. In repository Settings → Pages, choose **GitHub Actions** as the build and deployment source. Set the custom domain to `meeting-minutes.tech`. The build writes `dist/CNAME`, though GitHub Pages custom domain settings are authoritative for Actions deployments.
2. At the DNS provider, point the apex (`@`) to GitHub Pages with these four `A` records: `185.199.108.153`, `185.199.109.153`, `185.199.110.153`, `185.199.111.153`. Remove conflicting apex records. GitHub also documents optional IPv6 `AAAA` records and `ALIAS`/`ANAME` alternatives: [custom domain instructions](https://docs.github.com/en/pages/configuring-a-custom-domain-for-your-github-pages-site/managing-a-custom-domain-for-your-github-pages-site).
3. After DNS and the certificate settle, enable **Enforce HTTPS** in Settings → Pages. Confirm the domain and HTTPS status there before sharing the site.

Publishing a Release triggers the website workflow so the download URL, version, size, roadmap and Changelog refresh from GitHub Releases. Do not deploy a `--draft` build.
