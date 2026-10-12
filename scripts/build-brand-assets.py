"""Derive the build's brand files from the pack in assets/brand/.

- src-tauri/app-icon.png: assets/brand/app-icon.png is an opaque square (a
  rounded off-white plate on a white background with a drop shadow). Windows
  icons need a transparent outside, so the plate is cut out with a
  rounded-square mask and the shadow is dropped.
- src/assets/wordmark-dark.png: the wordmark cropped to its visible pixels and
  sized for the UI header.

See docs/adr/0004-branding-from-the-image-pack.md.

Usage (needs Pillow):
    python scripts/build-brand-assets.py
    pnpm tauri icon src-tauri/app-icon.png
"""

from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
BRAND = ROOT / "assets/brand"

# Plate bounds and corner radius measured on the 1254x1254 app-icon.png.
PLATE_BOX = (145, 149, 1109, 1106)
PLATE_RADIUS = 230
ICON_CANVAS = 1024
ICON_PLATE = 960  # leaves a 32px transparent margin
SUPERSAMPLE = 4

# The header shows the wordmark 32px tall; 3x covers high-DPI displays.
WORDMARK_HEIGHT = 96


def build_app_icon() -> Path:
    plate = Image.open(BRAND / "app-icon.png").convert("RGB").crop(PLATE_BOX)
    plate = plate.resize((ICON_PLATE, ICON_PLATE), Image.LANCZOS)

    # Anti-aliased mask, inset 2px so no shadow pixels survive at the edge.
    size = ICON_PLATE * SUPERSAMPLE
    inset = 2 * SUPERSAMPLE
    radius = PLATE_RADIUS * ICON_PLATE / (PLATE_BOX[2] - PLATE_BOX[0]) * SUPERSAMPLE
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        (inset, inset, size - 1 - inset, size - 1 - inset), radius=radius, fill=255
    )
    mask = mask.resize((ICON_PLATE, ICON_PLATE), Image.LANCZOS)

    icon = Image.new("RGBA", (ICON_CANVAS, ICON_CANVAS), (0, 0, 0, 0))
    offset = (ICON_CANVAS - ICON_PLATE) // 2
    icon.paste(plate, (offset, offset), mask)
    output = ROOT / "src-tauri/app-icon.png"
    icon.save(output)
    return output


def build_wordmark() -> Path:
    wordmark = Image.open(BRAND / "wordmark-dark.png").convert("RGBA")
    # Ignore near-transparent noise around the artwork when finding its bounds.
    visible = wordmark.getchannel("A").point(lambda a: 255 if a > 8 else 0)
    wordmark = wordmark.crop(visible.getbbox())
    width = round(wordmark.width * WORDMARK_HEIGHT / wordmark.height)
    wordmark = wordmark.resize((width, WORDMARK_HEIGHT), Image.LANCZOS)
    output = ROOT / "src/assets/wordmark-dark.png"
    wordmark.save(output, optimize=True)
    return output


def main() -> None:
    for output in (build_app_icon(), build_wordmark()):
        print(f"wrote {output.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
