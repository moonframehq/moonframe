#!/usr/bin/env python3
"""Lantern's pixel art: the sidebar's night scene (string lights, stars, a
jack-o'-lantern on a hill) and the app icon. Plain Python 3, no packages:

    python3 examples/lantern/art/draw.py

writes art/night.png (50 x 52 cells of 10 px, drawn at 250 x 260 points so
each cell is 5 points on a Retina screen) and icon.png (1024 px).
"""

import math
import os
import struct
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))


def hex_rgba(value, alpha=255):
    value = value.lstrip("#")
    return (int(value[0:2], 16), int(value[2:4], 16), int(value[4:6], 16), alpha)


def write_png(path, width, height, pixels):
    """pixels: rows of (r, g, b, a) tuples."""
    raw = bytearray()
    for row in pixels:
        raw.append(0)
        for r, g, b, a in row:
            raw += bytes((r, g, b, a))

    def chunk(kind, data):
        out = struct.pack(">I", len(data)) + kind + data
        return out + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    png += chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(png)


class Grid:
    def __init__(self, w, h):
        self.w, self.h = w, h
        self.cells = [[(0, 0, 0, 0)] * w for _ in range(h)]

    def set(self, x, y, color):
        if 0 <= x < self.w and 0 <= y < self.h:
            self.cells[y][x] = color

    def blend(self, x, y, color, amount):
        """Mix `color` over what is there, by `amount` (0 to 1)."""
        if not (0 <= x < self.w and 0 <= y < self.h):
            return
        r, g, b, a = self.cells[y][x]
        cr, cg, cb, _ = color
        if a == 0:
            self.cells[y][x] = (cr, cg, cb, int(255 * amount))
            return
        t = amount
        self.cells[y][x] = (
            int(r + (cr - r) * t),
            int(g + (cg - g) * t),
            int(b + (cb - b) * t),
            max(a, int(255 * amount)),
        )


# --------------------------------------------------------------- palette

RIND_EDGE = hex_rgba("#1e0605")
RIND_DARK = hex_rgba("#3a0d07")
RIND = hex_rgba("#521409")
RIND_MID = hex_rgba("#6a1c0b")
RIND_LIGHT = hex_rgba("#82280e")
SEAM = hex_rgba("#300a06")
STEM = hex_rgba("#5a5a22")
STEM_DARK = hex_rgba("#3a3a14")
STEM_LIGHT = hex_rgba("#7c7a34")
GLOW_EDGE = hex_rgba("#d8541a")
GLOW = hex_rgba("#f08a24")
GLOW_HOT = hex_rgba("#ffc24a")
GLOW_CORE = hex_rgba("#ffe08a")
LID = hex_rgba("#3a0c06")
PUPIL = hex_rgba("#1c0604")


def pumpkin(grid, cx, base):
    """A jack-o'-lantern centred on column `cx`, sitting on row `base`."""
    rx, ry = 19.5, 13.5
    cy = base - ry
    # Three lobes, the middle one tallest, so the outline dips at the seams.
    lobes = [(-8.5, 15.0, 12.5), (0.0, 12.0, 13.5), (8.5, 15.0, 12.5)]

    def inside(x, y):
        for ox, lrx, lry in lobes:
            dx = (x - ox) / lrx
            dy = (y) / lry
            if dx * dx + dy * dy <= 1.0:
                return True
        return False

    for gy in range(int(cy - ry - 1), int(base + 1)):
        for gx in range(int(cx - rx - 2), int(cx + rx + 3)):
            x = gx + 0.5 - cx
            y = gy + 0.5 - cy
            if not inside(x, y):
                continue
            edge = not (inside(x + 1, y) and inside(x - 1, y) and inside(x, y + 1) and inside(x, y - 1))
            if edge:
                grid.set(gx, gy, RIND_EDGE)
                continue
            # Light from the face inside and a moon at the top left.
            nx, ny = x / rx, y / ry
            shade = 0.55 - 0.35 * nx * nx - 0.25 * ny + 0.2 * (1 - abs(nx))
            if shade > 0.62:
                color = RIND_LIGHT
            elif shade > 0.42:
                color = RIND_MID
            elif shade > 0.2:
                color = RIND
            else:
                color = RIND_DARK
            # Seams between the lobes, curving with the body.
            for seam in (-13.0, -5.0, 5.0, 13.0):
                sx = seam * math.sqrt(max(0.0, 1 - (y / (ry + 1)) ** 2))
                if abs(x - sx) < 0.55:
                    color = SEAM
            grid.set(gx, gy, color)

    def face(x, y):
        """The carved face: eyes and grin glow, lids shade the eyes."""
        ax = abs(x)
        # Eyes: half-lidded almonds, slanting down toward the nose.
        ex, ey = 7.5, -3.5
        dx, dy = (ax - ex) / 4.2, (y - ey + (ax - ex) * 0.25) / 1.9
        if dx * dx + dy * dy <= 1.0:
            if dy < -0.15:
                return LID
            if abs(ax - (ex - 1.2)) < 0.8 and dy < 0.4:
                return PUPIL
            return GLOW_HOT if dy < 0.6 else GLOW
        # Nose: a small triangle.
        if -0.5 <= y <= 1.5 and ax <= 1.0 + (y + 0.5) * 0.6:
            return GLOW
        # Grin: between two curves, wider at the corners.
        top = 3.2 - 0.045 * ax * ax + 0.0016 * ax ** 3.6 / 10
        bottom = 7.6 - 0.018 * ax * ax
        if ax <= 12.5 and top <= y <= bottom and y >= 2.6:
            # Two teeth hanging from the top, one rising from the bottom.
            if y < top + 1.4 and (abs(ax - 5.0) < 1.0):
                return RIND_MID
            if y > bottom - 1.3 and ax < 1.0:
                return RIND_MID
            depth = (y - top) / max(0.5, bottom - top)
            if ax < 6 and 0.25 < depth < 0.85:
                return GLOW_CORE
            if ax < 9.5 and depth > 0.15:
                return GLOW_HOT
            return GLOW if ax < 11.5 else GLOW_EDGE
        return None

    for gy in range(int(cy - ry), int(base)):
        for gx in range(int(cx - rx), int(cx + rx + 1)):
            color = face(gx + 0.5 - cx, gy + 0.5 - cy)
            if color is not None:
                grid.set(gx, gy, color)

    # Stem, leaning right.
    top = int(cy - ry)
    stem = [(0, 0), (1, 0), (0, -1), (1, -1), (1, -2), (2, -2), (1, -3), (2, -3), (2, -4), (3, -4), (3, -5)]
    for sx, sy in stem:
        grid.set(int(cx) + sx - 1, top + sy, STEM)
    for sx, sy in [(0, 0), (0, -1), (1, -2), (1, -3), (2, -4)]:
        grid.set(int(cx) + sx - 1, top + sy, STEM_DARK)
    grid.set(int(cx) + 2, top - 5, STEM_LIGHT)
    grid.set(int(cx) + 1, top - 2, STEM_LIGHT)


def bulb(grid, x, y, core, mid, glow):
    """A hanging string light: a cap, a diamond bulb and its halo."""
    for dy in range(-4, 7):
        for dx in range(-4, 5):
            d = math.hypot(dx, (dy - 2) * 0.8)
            if d < 4.5:
                grid.blend(x + dx, y + dy, glow, max(0.0, 0.35 - d * 0.07))
    grid.set(x, y - 1, hex_rgba("#2a2a18"))
    grid.set(x, y, hex_rgba("#3a3a20"))
    for dx, dy in [(0, 1), (-1, 2), (0, 2), (1, 2), (-1, 3), (0, 3), (1, 3), (0, 4)]:
        grid.set(x + dx, y + dy, mid)
    grid.set(x, y + 2, core)
    grid.set(x, y + 3, core)
    # The sparkle: a cross of light around the bulb.
    grid.blend(x - 2, y + 2, mid, 0.55)
    grid.blend(x + 2, y + 2, mid, 0.55)
    grid.blend(x, y + 5, mid, 0.45)


def night(width=50, height=52):
    grid = Grid(width, height)
    # The wire: a sagging zigzag, darker olive.
    wire = hex_rgba("#34341a")
    for x in range(width):
        y = 2 + (1 if (x // 4) % 3 == 1 else 0) + (1 if (x // 4) % 3 == 2 else 0) * 0
        y = 2 + round(1.2 * abs(math.sin((x + 2) * math.pi / 16.0)))
        grid.set(x, y, wire)
    bulb(grid, 5, 5, hex_rgba("#ffe0a0"), hex_rgba("#ff5a14"), hex_rgba("#ff5a14"))
    bulb(grid, 25, 5, hex_rgba("#f2d4ff"), hex_rgba("#9b3cf0"), hex_rgba("#9b3cf0"))
    bulb(grid, 44, 5, hex_rgba("#d8ffe8"), hex_rgba("#1fbf72"), hex_rgba("#1fbf72"))
    # Stars.
    star = hex_rgba("#4e4472")
    bright = hex_rgba("#7a6fa8")
    for x, y, c in [(2, 13, star), (11, 11, bright), (17, 15, star), (28, 12, star), (37, 13, bright),
                    (46, 16, star), (7, 20, star), (33, 19, star), (21, 9, star), (48, 24, bright),
                    (1, 27, star), (42, 9, star), (14, 25, star)]:
        grid.set(x, y, c)
    # The hill behind and the ground under the pumpkin.
    for x in range(width):
        hill = 38 + round(2.5 * math.cos((x - 8) * math.pi / 30.0))
        for y in range(hill, height):
            grid.set(x, y, hex_rgba("#1a1328") if y < 44 else hex_rgba("#0c0812"))
        grid.set(x, hill, hex_rgba("#2a2040"))
    pumpkin(grid, 25, 49)
    # A shadow on the ground under it.
    for x in range(8, 43):
        grid.blend(x, 49, hex_rgba("#000000"), 0.5)
    return grid


def scale(grid, cell):
    rows = []
    for row in grid.cells:
        line = []
        for color in row:
            line.extend([color] * cell)
        for _ in range(cell):
            rows.append(line)
    return rows


def icon():
    size = 1024
    sprite = Grid(46, 40)
    pumpkin(sprite, 23, 37)
    cell = 16
    ox = (size - sprite.w * cell) // 2
    oy = (size - sprite.h * cell) // 2 + 30
    bg_top, bg_bottom = hex_rgba("#2a1440"), hex_rgba("#0e0618")
    rows = []
    x0, y0, x1, y1, radius = 100, 100, 924, 924, 185
    for y in range(size):
        t = (y - y0) / (y1 - y0)
        bg = tuple(int(bg_top[i] + (bg_bottom[i] - bg_top[i]) * max(0, min(1, t))) for i in range(3)) + (255,)
        line = []
        for x in range(size):
            # Inside the rounded square?
            cx = min(max(x, x0 + radius), x1 - radius)
            cy = min(max(y, y0 + radius), y1 - radius)
            if (x - cx) ** 2 + (y - cy) ** 2 > radius * radius or not (x0 <= x <= x1 and y0 <= y <= y1):
                line.append((0, 0, 0, 0))
                continue
            sx, sy = (x - ox) // cell, (y - oy) // cell
            color = None
            if 0 <= sx < sprite.w and 0 <= sy < sprite.h:
                c = sprite.cells[sy][sx]
                if c[3]:
                    color = c
            line.append(color or bg)
        rows.append(line)
    return size, rows


if __name__ == "__main__":
    scene = night()
    cell = 10
    write_png(os.path.join(HERE, "night.png"), scene.w * cell, scene.h * cell, scale(scene, cell))
    size, rows = icon()
    write_png(os.path.join(HERE, "..", "icon.png"), size, size, rows)
    print("wrote art/night.png and icon.png")
