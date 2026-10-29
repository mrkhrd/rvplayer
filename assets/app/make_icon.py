"""Draws the RV Player icon: a blue tile with a play triangle, a route and a map pin.

Writes rvplayer.ico (16-256 px, each size drawn on its own so the small ones stay sharp) and
rvplayer.png (256 px) next to this script. Needs Pillow:  python make_icon.py
"""
import io
import math
import struct
from pathlib import Path

from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parent
SIZES = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256]
SUPER = 8  # drawn this many times larger, then reduced (anti-aliasing)


def lerp(a, b, t):
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


def draw(size):
    s = size * SUPER
    img = Image.new("RGBA", (s, s), (0, 0, 0, 0))

    # The tile: a vertical gradient in a rounded square.
    margin = s * (0.03 if size >= 32 else 0.0)
    radius = s * 0.2
    grad = Image.new("RGBA", (s, s))
    gd = ImageDraw.Draw(grad)
    top, bottom = (40, 110, 190), (14, 42, 82)
    for y in range(s):
        gd.line([(0, y), (s, y)], fill=lerp(top, bottom, y / s) + (255,))
    mask = Image.new("L", (s, s), 0)
    ImageDraw.Draw(mask).rounded_rectangle([margin, margin, s - margin, s - margin], radius, fill=255)
    img.paste(grad, (0, 0), mask)
    # The route: a winding line from the bottom left to the pin (larger sizes only), blended
    # onto the tile from a layer of its own.
    if size >= 40:
        pts = []
        for i in range(41):
            t = i / 40
            x = 0.12 + 0.62 * t
            y = 0.86 - 0.52 * t + 0.07 * math.sin(t * math.pi * 2.2)
            pts.append((x * s, y * s))
        route = Image.new("RGBA", (s, s), (0, 0, 0, 0))
        ImageDraw.Draw(route).line(pts, fill=(255, 130, 130, 170), width=round(s * 0.05), joint="curve")
        img.alpha_composite(route)
    d = ImageDraw.Draw(img)

    # The play triangle.
    cx, cy, h = 0.44 * s, 0.55 * s, 0.44 * s
    w = h * math.sqrt(3) / 2
    d.polygon([(cx - w / 3, cy - h / 2), (cx - w / 3, cy + h / 2), (cx + 2 * w / 3, cy)], fill=(255, 255, 255, 255))

    # The map pin, top right, with a ring of the tile colour around it.
    px, py, r = 0.72 * s, 0.30 * s, 0.15 * s
    ring = s * 0.035
    if size >= 32:
        tip = (px, py + r * 2.1)
        for grow, colour in ((ring, (22, 60, 110, 255)), (0, (229, 57, 53, 255))):
            d.ellipse([px - r - grow, py - r - grow, px + r + grow, py + r + grow], fill=colour)
            a = math.radians(32)
            d.polygon([(px - (r + grow) * math.cos(a), py + (r + grow) * math.sin(a)),
                       (px + (r + grow) * math.cos(a), py + (r + grow) * math.sin(a)),
                       (tip[0], tip[1] + grow * 1.4)], fill=colour)
        d.ellipse([px - r * 0.42, py - r * 0.42, px + r * 0.42, py + r * 0.42], fill=(255, 255, 255, 255))
    else:
        r *= 1.15
        d.ellipse([px - r - ring, py - r - ring, px + r + ring, py + r + ring], fill=(22, 60, 110, 255))
        d.ellipse([px - r, py - r, px + r, py + r], fill=(229, 57, 53, 255))

    return img.resize((size, size), Image.LANCZOS)


def write_ico(path, images):
    """An ICO of PNG images (Windows Vista and later read PNG entries at every size)."""
    blobs = []
    for im in images:
        buf = io.BytesIO()
        im.save(buf, format="PNG")
        blobs.append(buf.getvalue())
    out = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    for im, blob in zip(images, blobs):
        w, h = im.size
        out += struct.pack("<BBBBHHII", w % 256, h % 256, 0, 0, 1, 32, len(blob), offset)
        offset += len(blob)
    path.write_bytes(out + b"".join(blobs))


if __name__ == "__main__":
    images = [draw(n) for n in SIZES]
    write_ico(HERE / "rvplayer.ico", images)
    images[-1].save(HERE / "rvplayer.png")
    print("written", HERE / "rvplayer.ico")
