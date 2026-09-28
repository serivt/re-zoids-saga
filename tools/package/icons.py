"""Builds the program's icons from assets/icons/re-zoids-saga.png.

    python3 tools/package/icons.py

Writes, next to it: re-zoids-saga.ico (Windows: 16 to 256 pixels),
re-zoids-saga.icns (macOS: 16 to 512 pixels, never enlarged) and
re-zoids-saga-128.rgba (the window's icon: 128x128 pixels, 8-bit RGBA rows,
top to bottom); and the Android app's launcher icons (48 to 192 pixels, one
per screen density). Needs Pillow, and macOS's iconutil for the .icns.
"""

import subprocess
import tempfile
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
FOLDER = ROOT / "assets" / "icons"
WINDOW = 128
ANDROID_RES = ROOT / "apps" / "android" / "project" / "app" / "src" / "main" / "res"
ANDROID_DENSITIES = {"mdpi": 48, "hdpi": 72, "xhdpi": 96, "xxhdpi": 144, "xxxhdpi": 192}


def main():
    master = Image.open(FOLDER / "re-zoids-saga.png").convert("RGBA")
    side = master.width
    ico_sizes = [(s, s) for s in (16, 24, 32, 48, 64, 128, 256) if s <= side]
    master.save(FOLDER / "re-zoids-saga.ico", sizes=ico_sizes)
    with tempfile.TemporaryDirectory() as temporary:
        iconset = Path(temporary) / "re-zoids-saga.iconset"
        iconset.mkdir()
        for base in (16, 32, 128, 256, 512):
            for scale, suffix in ((1, ""), (2, "@2x")):
                size = base * scale
                if size <= side:
                    master.resize((size, size), Image.LANCZOS).save(
                        iconset / f"icon_{base}x{base}{suffix}.png")
        subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o",
                        str(FOLDER / "re-zoids-saga.icns")], check=True)
    window = master.resize((WINDOW, WINDOW), Image.LANCZOS)
    (FOLDER / f"re-zoids-saga-{WINDOW}.rgba").write_bytes(window.tobytes())
    for density, size in ANDROID_DENSITIES.items():
        folder = ANDROID_RES / f"mipmap-{density}"
        folder.mkdir(parents=True, exist_ok=True)
        master.resize((size, size), Image.LANCZOS).save(folder / "ic_launcher.png")


main()
