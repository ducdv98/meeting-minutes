# Branding

The brand pack in `assets/brand/` holds the original files and is never edited. The build uses files derived from it: `src-tauri/app-icon.png` and `src-tauri/icons/` for the app, and `src/assets/` for the UI. To rebuild them after changing the pack:

```sh
python scripts/build-brand-assets.py      # needs Pillow
pnpm tauri icon src-tauri/app-icon.png    # then keep only the files listed in tauri.conf.json
```

See `docs/adr/0004-branding-from-the-image-pack.md`.
