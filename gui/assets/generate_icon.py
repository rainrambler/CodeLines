#!/usr/bin/env python3
"""Generate the CodeLines app icon.

One geometry spec drives every artefact in this folder:

    icon.svg          vector master (same geometry, hand-editable)
    icon-1024.png     largest raster
    icon-512..16.png  window / launcher sizes
    icon.ico          Windows multi-size icon (exe resource + taskbar)

Run from the repository root:

    python gui/assets/generate_icon.py

Requires Pillow and NumPy (build-time only; the app ships the PNGs).
"""

from __future__ import annotations

from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

ASSETS = Path(__file__).resolve().parent

# Design space: every coordinate below is in 1024x1024 units.
SIZE = 1024
SS = 4  # supersampling factor for the raster master
W = SIZE * SS

TILE_RADIUS = 224

BG_STOPS = [
    (0.00, "#28324B"),
    (0.55, "#171B26"),
    (1.00, "#0D0F15"),
]
GLOW_COLOR = "#4F9CF7"

GUTTER = (214, 194, 16, 636)  # x, y, w, h
GUTTER_COLOR = "#3E4757"

# x, y, w, h, base colour — one line per category the app reports on.
BARS = [
    (286, 194, 540, 132, "#4F9CF7"),  # code
    (286, 446, 384, 132, "#3FC18F"),  # comments
    (286, 698, 468, 132, "#F2B541"),  # mixed
]
BAR_RADIUS = 66

# Raster outputs written next to this script.
PNG_SIZES = [1024, 512, 256, 128, 64, 48, 32, 24, 16]
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]


def rgb(hex_color: str) -> np.ndarray:
    hex_color = hex_color.lstrip("#")
    return np.array([int(hex_color[i : i + 2], 16) for i in (0, 2, 4)], dtype=np.float32)


def shade(hex_color: str, amount: float) -> np.ndarray:
    """Mix towards white (amount > 0) or black (amount < 0)."""
    color = rgb(hex_color)
    target = np.array([255.0, 255.0, 255.0] if amount > 0 else [0.0, 0.0, 0.0])
    return color + (target - color) * abs(amount)


def px(value: float) -> int:
    return int(round(value * SS))


def rounded_mask(box: tuple[float, float, float, float], radius: float) -> Image.Image:
    x, y, w, h = box
    mask = Image.new("L", (W, W), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        [px(x), px(y), px(x + w) - 1, px(y + h) - 1], radius=px(radius), fill=255
    )
    return mask


def linear_box_image(w_px: int, h_px: int, top: np.ndarray, bottom: np.ndarray) -> Image.Image:
    t = np.linspace(0.0, 1.0, h_px, dtype=np.float32)[:, None, None]
    row = top[None, None, :] * (1.0 - t) + bottom[None, None, :] * t
    arr = np.repeat(row, w_px, axis=1)
    return Image.fromarray(arr.astype(np.uint8), "RGB")


def paint_box(
    canvas: Image.Image,
    box: tuple[float, float, float, float],
    radius: float,
    top: np.ndarray,
    bottom: np.ndarray,
) -> None:
    x, y, w, h = box
    x0, y0 = px(x), px(y)
    w_px, h_px = px(w), px(h)
    canvas.paste(
        linear_box_image(w_px, h_px, top, bottom),
        (x0, y0),
        rounded_mask(box, radius).crop((x0, y0, x0 + w_px, y0 + h_px)),
    )


def background() -> Image.Image:
    yy, xx = np.mgrid[0:W, 0:W].astype(np.float32) / (W - 1)
    t = np.clip(0.28 * xx + 0.72 * yy, 0.0, 1.0)

    positions = np.array([stop[0] for stop in BG_STOPS], dtype=np.float32)
    colors = np.stack([rgb(stop[1]) for stop in BG_STOPS])
    pixels = np.stack([np.interp(t, positions, colors[:, c]) for c in range(3)], axis=-1)

    # Soft accent glow in the upper-left corner.
    dist = np.sqrt((xx - 0.16) ** 2 + (yy - 0.02) ** 2) / 0.80
    alpha = (np.clip(1.0 - dist, 0.0, 1.0) ** 2.2 * 0.42)[..., None]
    pixels = pixels * (1.0 - alpha) + rgb(GLOW_COLOR) * alpha

    return Image.fromarray(pixels.astype(np.uint8), "RGB")


def render_master() -> Image.Image:
    canvas = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    canvas.paste(background(), (0, 0), rounded_mask((0, 0, SIZE, SIZE), TILE_RADIUS))

    paint_box(canvas, GUTTER, GUTTER[2] / 2, rgb(GUTTER_COLOR), rgb(GUTTER_COLOR))
    for x, y, w, h, color in BARS:
        paint_box(canvas, (x, y, w, h), BAR_RADIUS, shade(color, 0.14), shade(color, -0.10))

    stroke = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    inset = px(2)
    ImageDraw.Draw(stroke).rounded_rectangle(
        [inset, inset, W - inset - 1, W - inset - 1],
        radius=px(TILE_RADIUS - 2),
        outline=(255, 255, 255, 22),
        width=px(3),
    )
    canvas = Image.alpha_composite(canvas, stroke)

    return canvas.resize((SIZE, SIZE), Image.LANCZOS)


def write_svg() -> None:
    defs = ['    <linearGradient id="bg" x1="0.14" y1="0" x2="0.72" y2="1">']
    defs += [f'      <stop offset="{offset}" stop-color="{color}"/>' for offset, color in BG_STOPS]
    defs += [
        "    </linearGradient>",
        '    <radialGradient id="glow" cx="0.16" cy="0.02" r="0.8">',
        f'      <stop offset="0" stop-color="{GLOW_COLOR}" stop-opacity="0.42"/>',
        f'      <stop offset="0.55" stop-color="{GLOW_COLOR}" stop-opacity="0.10"/>',
        f'      <stop offset="1" stop-color="{GLOW_COLOR}" stop-opacity="0"/>',
        "    </radialGradient>",
    ]
    for index, (*_, color) in enumerate(BARS):
        top = "#%02X%02X%02X" % tuple(int(v) for v in shade(color, 0.14))
        bottom = "#%02X%02X%02X" % tuple(int(v) for v in shade(color, -0.10))
        defs += [
            f'    <linearGradient id="bar{index}" x1="0" y1="0" x2="0" y2="1">',
            f'      <stop offset="0" stop-color="{top}"/>',
            f'      <stop offset="1" stop-color="{bottom}"/>',
            "    </linearGradient>",
        ]

    gx, gy, gw, gh = GUTTER
    bars = "\n".join(
        f'  <rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{BAR_RADIUS}" fill="url(#bar{index})"/>'
        for index, (x, y, w, h, _) in enumerate(BARS)
    )

    svg = f"""<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024" role="img" aria-label="CodeLines">
  <title>CodeLines</title>
  <defs>
{chr(10).join(defs)}
    <clipPath id="tile">
      <rect width="1024" height="1024" rx="{TILE_RADIUS}"/>
    </clipPath>
  </defs>
  <g clip-path="url(#tile)">
    <rect width="1024" height="1024" fill="url(#bg)"/>
    <rect width="1024" height="1024" fill="url(#glow)"/>
  </g>
  <rect x="2" y="2" width="1020" height="1020" rx="{TILE_RADIUS - 2}" fill="none" stroke="#FFFFFF" stroke-opacity="0.09" stroke-width="3"/>
  <rect x="{gx}" y="{gy}" width="{gw}" height="{gh}" rx="{gw // 2}" fill="{GUTTER_COLOR}"/>
{bars}
</svg>
"""
    (ASSETS / "icon.svg").write_text(svg, encoding="utf-8", newline="\n")


def main() -> None:
    master = render_master()
    for size in PNG_SIZES:
        image = master if size == SIZE else master.resize((size, size), Image.LANCZOS)
        image.save(ASSETS / f"icon-{size}.png", optimize=True)

    master.resize((256, 256), Image.LANCZOS).save(
        ASSETS / "icon.ico", format="ICO", sizes=[(size, size) for size in ICO_SIZES]
    )
    write_svg()


if __name__ == "__main__":
    main()
