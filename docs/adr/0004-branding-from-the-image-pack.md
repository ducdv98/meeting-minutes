# Branding comes from the image pack in `assets/brand/`

All app branding uses the image pack in `assets/brand/` (mark, wordmark and app icon, in dark and white variants). The pack holds the originals: it is committed as delivered, never edited by hand, and never imported by app code. Everything the build ships is derived from it by `scripts/build-brand-assets.py` and committed next to the code that uses it:

| Derived file | Used for |
| --- | --- |
| `src-tauri/app-icon.png` | Icon source for `pnpm tauri icon` |
| `src-tauri/icons/` | App, installer, taskbar and tray icon |
| `src/assets/wordmark-dark.png` | Library screen header |

- **App, installer, taskbar and tray icon**: the light plate from `assets/brand/app-icon.png`. That file is an opaque square with a white background and a drop shadow, which would show as a white box on a dark Windows taskbar. The script cuts the plate out onto a transparent 1024px canvas. Of the files `pnpm tauri icon` generates, only the five listed in `tauri.conf.json` are kept (Windows only; no mobile or Store sizes). The tray uses the same icon as the window, because the light plate shows up on both light and dark taskbars.
- **In-app header**: `wordmark-dark.png`, cropped to the artwork and sized for a 32px-tall header at 3x, because the UI is light-only for now. The full-size original would add about 265 KB to the UI bundle.
- **Reserved for later**: the `-white` and `-on-black`/`-on-charcoal` variants, for a dark theme or a dark tray icon; `mark-white-faint.png`, for empty states and watermarks.

## Considered Options

- **Transparent `mark-dark.png` as the icon**: no plate, but the dark mark nearly disappears on Windows 11's default dark taskbar.
- **Using `assets/brand/app-icon.png` unchanged**: simplest, but the white square corners show around the rounded plate.
- **Importing the pack directly from the UI**: no build step, but it mixes originals with shipped files and bundles full-size images.

## Consequences

- Changing the brand means replacing files in `assets/brand/` and rerunning `python scripts/build-brand-assets.py` (needs Pillow) and `pnpm tauri icon src-tauri/app-icon.png`. If the new `app-icon.png` has a different plate size or corner radius, update `PLATE_BOX` and `PLATE_RADIUS` in the script.
- Generated icons are committed, so CI does not need Pillow.
- SVG versions of the mark and wordmark would let the script render the icon at every size instead of cutting it from a raster image.
