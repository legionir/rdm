#!/usr/bin/env python3
"""Generate the rdm application icon (no third-party libraries).

Reproducible asset pipeline for `rdm-gui`:

    python3 rdm-gui/assets/generate_icon.py

Writes, next to this script:

    icon.rgba     64x64 RGBA bytes  — window icon (egui) and tray icon
    icon.png      256x256 PNG       — documentation / store art
    icon.ico      16,24,32,48,64,   — Windows executable icon (build.rs)
                  128,256 BMP+alpha entries

Design: the app palette's blue (`#1D4ED8` light-theme accent) as a rounded
square with a vertical gradient, and a light-theme-friendly white "download
into a tray" glyph (`#7CB3FF` dark-theme accent is used for the shaft
highlight so the icon sits in the same family as the UI tokens).
Everything is rasterised at 4x and box-filtered down, so the 16 px version is
still crisp.
"""

from __future__ import annotations

import pathlib
import struct
import zlib

SS = 4  # supersampling factor
DESIGN = 256  # design grid in units


# --------------------------------------------------------------------- shapes
def rounded_rect_cov(px: float, py: float, x: float, y: float, w: float, h: float, r: float) -> float:
    """Coverage (0..1) of a rounded rectangle whose centre lies on (px, py).

    `px`/`py` are the *centre* of the sample cell; the shape spans
    (x, y) .. (x + w, y + h).  Hard edges are fine: the caller supersamples.
    """
    cx = x + w / 2.0
    cy = y + h / 2.0
    dx = abs(px - cx) - (w / 2.0 - r)
    dy = abs(py - cy) - (h / 2.0 - r)
    if dx <= 0.0 and dy <= 0.0:
        return 1.0
    d = ((max(dx, 0.0)) ** 2 + (max(dy, 0.0)) ** 2) ** 0.5 - r
    return 1.0 if d <= 0.0 else 0.0


def triangle_cov(px: float, py: float, a: tuple[float, float], b: tuple[float, float], c: tuple[float, float]) -> float:
    """Coverage for a triangle (half-plane test, samples at pixel centres)."""
    def edge(p, q, s) -> float:
        return (q[0] - p[0]) * (s[1] - p[1]) - (q[1] - p[1]) * (s[0] - p[0])

    p = (px, py)
    d1 = edge(a, b, p)
    d2 = edge(b, c, p)
    d3 = edge(c, a, p)
    inside = (d1 >= 0 and d2 >= 0 and d3 >= 0) or (d1 <= 0 and d2 <= 0 and d3 <= 0)
    return 1.0 if inside else 0.0


def mix(dst: list[float], src: tuple[float, float, float, float], cov: float) -> None:
    """Alpha-composite `src` (r,g,b,a in 0..1) over `dst` with coverage `cov`."""
    sa = src[3] * cov
    if sa <= 0.0:
        return
    da = dst[3]
    out_a = sa + da * (1.0 - sa)
    if out_a <= 0.0:
        dst[:] = [0.0, 0.0, 0.0, 0.0]
        return
    for i in range(3):
        dst[i] = (src[i] * sa + dst[i] * da * (1.0 - sa)) / out_a
    dst[3] = out_a


# ------------------------------------------------------------------- painting
BG_TOP = (0x1D / 255, 0x4E / 255, 0xD8 / 255)      # palette accent (light theme)
BG_BOTTOM = (0x0B / 255, 0x27 / 255, 0x77 / 255)   # deeper blue
GLYPH = (1.0, 1.0, 1.0)                            # white
SHAFT = (0x7C / 255, 0xB3 / 255, 0xFF / 255)       # palette accent (dark theme)


def render(size: int) -> bytearray:
    """Render one icon at `size` px (RGBA, top-down rows)."""
    n = size * SS
    scale = n / DESIGN
    out = bytearray(size * size * 4)

    for py in range(size):
        for px in range(size):
            acc = [0.0, 0.0, 0.0, 0.0]
            for sy in range(SS):
                for sx in range(SS):
                    # sample point in design units
                    x = ((px * SS + sx) + 0.5) / scale
                    y = ((py * SS + sy) + 0.5) / scale

                    # 1. rounded-square background with a vertical gradient
                    cov = rounded_rect_cov(x, y, 8.0, 8.0, 240.0, 240.0, 54.0)
                    if cov > 0.0:
                        t = min(max((y - 8.0) / 240.0, 0.0), 1.0)
                        r = BG_TOP[0] + (BG_BOTTOM[0] - BG_TOP[0]) * t
                        g = BG_TOP[1] + (BG_BOTTOM[1] - BG_TOP[1]) * t
                        b = BG_TOP[2] + (BG_BOTTOM[2] - BG_TOP[2]) * t
                        mix(acc, (r, g, b, 1.0), cov)

                    # 2. soft highlight in the upper third (glass feel)
                    hl = rounded_rect_cov(x, y, 8.0, 8.0, 240.0, 96.0, 40.0)
                    if hl > 0.0:
                        mix(acc, (1.0, 1.0, 1.0, 0.07), hl)

                    # 3. download arrow: shaft (slightly tinted) + head
                    if rounded_rect_cov(x, y, 108.0, 62.0, 40.0, 92.0, 18.0) > 0.0:
                        mix(acc, (*SHAFT, 1.0), 1.0)
                    if triangle_cov(x, y, (74.0, 146.0), (182.0, 146.0), (128.0, 202.0)) > 0.0:
                        mix(acc, (*GLYPH, 1.0), 1.0)

                    # 4. tray/base bar
                    if rounded_rect_cov(x, y, 64.0, 206.0, 128.0, 26.0, 13.0) > 0.0:
                        mix(acc, (*GLYPH, 1.0), 1.0)

            i = (py * size + px) * 4
            out[i] = round(acc[0] * 255.0)
            out[i + 1] = round(acc[1] * 255.0)
            out[i + 2] = round(acc[2] * 255.0)
            out[i + 3] = round(acc[3] * 255.0)
    return out


# ------------------------------------------------------------------- encoders
def write_png(path: pathlib.Path, size: int, rgba: bytes) -> None:
    raw = b"".join(b"\x00" + rgba[y * size * 4 : (y + 1) * size * 4] for y in range(size))

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )
    path.write_bytes(png)


def write_ico(path: pathlib.Path, images: list[tuple[int, bytes]]) -> None:
    """32bpp BMP entries with an AND mask — accepted by Windows for all sizes."""
    entries, blobs = [], b""
    offset = 6 + 16 * len(images)
    for size, rgba in images:
        header = struct.pack("<IiiHHIIiiII", 40, size, size * 2, 1, 32, 0, 0, 0, 0, 0, 0)
        xor = bytearray()
        for y in range(size - 1, -1, -1):  # bottom-up BGRA
            row = rgba[y * size * 4 : (y + 1) * size * 4]
            for x in range(size):
                r, g, b, a = row[x * 4 : x * 4 + 4]
                xor += bytes((b, g, r, a))
        mask_row = ((size + 31) // 32) * 4  # 1bpp rows padded to 4 bytes
        and_mask = bytes(mask_row * size)
        blob = header + bytes(xor) + and_mask
        entries.append(struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(blob), offset))
        blobs += blob
        offset += len(blob)

    ico = struct.pack("<HHH", 0, 1, len(images)) + b"".join(entries) + blobs
    path.write_bytes(ico)


def main() -> None:
    here = pathlib.Path(__file__).resolve().parent
    sizes = [16, 24, 32, 48, 64, 128, 256]
    rendered = {size: bytes(render(size)) for size in sizes}

    (here / "icon.rgba").write_bytes(rendered[64])
    write_png(here / "icon.png", 256, rendered[256])
    write_ico(here / "icon.ico", [(size, rendered[size]) for size in sizes])

    print(f"icon.rgba  {len(rendered[64])} bytes (64x64 RGBA)")
    print(f"icon.png   {(here / 'icon.png').stat().st_size} bytes (256x256)")
    print(f"icon.ico   {(here / 'icon.ico').stat().st_size} bytes ({', '.join(map(str, sizes))})")


if __name__ == "__main__":
    main()
