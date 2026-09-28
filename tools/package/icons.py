"""Builds the program's icons from assets/icons/re-zoids-saga.png.

    python3 tools/package/icons.py

Writes, next to it: re-zoids-saga.ico (Windows: 16 to 256 pixels),
re-zoids-saga.icns (macOS: 16 to 512 pixels, never enlarged) and
re-zoids-saga-128.rgba (the window's icon: 128x128 pixels, 8-bit RGBA rows,
top to bottom). Needs Pillow, and macOS's iconutil for the .icns.
"""

import subprocess
import tempfile
from pathlib import Path

from PIL import Image

FOLDER = Path(__file__).resolve().parents[2] / "assets" / "icons"
WINDOW = 128


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


main()
