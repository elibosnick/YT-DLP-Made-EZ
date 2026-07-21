#!/usr/bin/env python3
"""
Generates every icon Tauri's bundler needs, from one vector definition.

Run this instead of hand-editing PNGs, so all sizes stay in sync:

    python3 scripts/generate-icons.py

Design intent: a downward arrow landing on a tray, on a rounded blue square. The shape
is deliberately chunky — it has to stay legible at 16px in a Windows taskbar, where
thin strokes disappear entirely.
"""

import struct
import sys
from pathlib import Path

try:
    from PIL import Image, ImageDraw
except ImportError:
    sys.exit("Pillow is required:  pip install Pillow")

ICON_DIR = Path(__file__).resolve().parent.parent / "src-tauri" / "icons"

BG = (26, 95, 208, 255)      # matches --accent in App.css
FG = (255, 255, 255, 255)


def draw_icon(size: int) -> Image.Image:
    # Supersample 4x and downscale: gives clean antialiasing on the rounded corners
    # and the arrow diagonals without needing a real vector renderer.
    scale = 4
    s = size * scale
    img = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    d.rounded_rectangle([0, 0, s - 1, s - 1], radius=int(s * 0.22), fill=BG)

    cx = s / 2
    shaft_w = s * 0.13
    shaft_top = s * 0.20
    shaft_bottom = s * 0.52
    d.rectangle([cx - shaft_w / 2, shaft_top, cx + shaft_w / 2, shaft_bottom], fill=FG)

    head_w = s * 0.34
    head_tip = s * 0.70
    d.polygon(
        [(cx - head_w / 2, shaft_bottom), (cx + head_w / 2, shaft_bottom), (cx, head_tip)],
        fill=FG,
    )

    tray_h = s * 0.075
    tray_y = s * 0.775
    d.rounded_rectangle(
        [s * 0.24, tray_y, s * 0.76, tray_y + tray_h],
        radius=tray_h / 2,
        fill=FG,
    )

    return img.resize((size, size), Image.LANCZOS)


def write_icns(path: Path, sizes: dict[str, int]) -> None:
    """
    Minimal ICNS writer.

    The format is: 'icns', total length, then one chunk per size — a 4-byte OSType, a
    4-byte big-endian length, and the payload. Modern OSTypes accept a PNG payload
    directly, so no legacy bitmap packing is needed. Written by hand because
    `iconutil` is macOS-only and this has to run in Linux CI too.
    """
    import io

    chunks = b""
    for ostype, size in sizes.items():
        buf = io.BytesIO()
        draw_icon(size).save(buf, format="PNG")
        payload = buf.getvalue()
        chunks += ostype.encode("ascii") + struct.pack(">I", len(payload) + 8) + payload

    path.write_bytes(b"icns" + struct.pack(">I", len(chunks) + 8) + chunks)


def main() -> None:
    ICON_DIR.mkdir(parents=True, exist_ok=True)

    # Names Tauri looks for, plus the extra sizes Linux desktop environments use.
    for name, size in {
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
        "icon.png": 512,
        "Square30x30Logo.png": 30,
        "Square44x44Logo.png": 44,
        "Square71x71Logo.png": 71,
        "Square89x89Logo.png": 89,
        "Square107x107Logo.png": 107,
        "Square142x142Logo.png": 142,
        "Square150x150Logo.png": 150,
        "Square284x284Logo.png": 284,
        "Square310x310Logo.png": 310,
        "StoreLogo.png": 50,
    }.items():
        draw_icon(size).save(ICON_DIR / name, format="PNG")

    # Windows wants several sizes inside one .ico so Explorer can pick per context.
    draw_icon(256).save(
        ICON_DIR / "icon.ico",
        format="ICO",
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )

    write_icns(
        ICON_DIR / "icon.icns",
        {"icp4": 16, "icp5": 32, "icp6": 64, "ic07": 128, "ic08": 256, "ic09": 512},
    )

    print(f"Wrote icons to {ICON_DIR}")


if __name__ == "__main__":
    main()
