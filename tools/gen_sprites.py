#!/usr/bin/env python3
"""Generate all pixel art used by ui/main.slint:

- ui/font/{big,small}/*.png   clock (4x) and temperature (3x) glyphs, 5x7 font
- ui/font/text/*.png + ui/font/text.slint   uppercase text font (2x) for the Wi-Fi setup screen
- ui/web/pixel.ttf            the same font as a TrueType web font, for the setup web page
- ui/weather/*.png            weather icons (12x12 art, 3x)
- ui/sky/moon.png             the moon that circles the screen at night (10x10 art, 2x)
- ui/bins/*.png               wheelie bins for the bin-night reminder (16x20 art, 3x)

Art is drawn on a small grid, then scaled up with nearest-neighbour so Slint can
blit it 1:1 (the software renderer has no "pixelated" scaling).

No third-party deps: PNGs are written with zlib + struct.
Replace any PNG with hand-drawn art any time - keep the name and size.

    python3 tools/gen_sprites.py
"""

import math
import os
import struct
import zlib

UI_DIR = os.path.join(os.path.dirname(__file__), "..", "ui")
CLEAR = (0, 0, 0, 0)


def write_png(path, px, scale):
    height, width = len(px) * scale, len(px[0]) * scale
    raw = bytearray()
    for y in range(height):
        raw.append(0)  # filter: none
        for x in range(width):
            raw.extend(px[y // scale][x // scale])

    def chunk(tag, data):
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    png += chunk(b"IEND", b"")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(png)


# ---------------------------------------------------------------- font

GLYPHS = {
    "0": [".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###."],
    "1": ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."],
    "2": [".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"],
    "3": ["####.", "....#", "....#", ".###.", "....#", "....#", "####."],
    "4": ["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."],
    "5": ["#####", "#....", "####.", "....#", "....#", "#...#", ".###."],
    "6": [".###.", "#....", "#....", "####.", "#...#", "#...#", ".###."],
    "7": ["#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."],
    "8": [".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."],
    "9": [".###.", "#...#", "#...#", ".####", "....#", "....#", ".###."],
    "colon": [".", ".", "#", ".", "#", ".", "."],
    "minus": ["...", "...", "...", "###", "...", "...", "..."],
    "degree": [".#.", "#.#", ".#.", "...", "...", "...", "..."],
}
# Uppercase text font (5x7). Digits come from GLYPHS above.
LETTERS = {
    "A": [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
    "B": ["####.", "#...#", "#...#", "####.", "#...#", "#...#", "####."],
    "C": [".###.", "#...#", "#....", "#....", "#....", "#...#", ".###."],
    "D": ["####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####."],
    "E": ["#####", "#....", "#....", "####.", "#....", "#....", "#####"],
    "F": ["#####", "#....", "#....", "####.", "#....", "#....", "#...."],
    "G": [".###.", "#...#", "#....", "#.###", "#...#", "#...#", ".####"],
    "H": ["#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
    "I": [".###.", "..#..", "..#..", "..#..", "..#..", "..#..", ".###."],
    "J": ["..###", "...#.", "...#.", "...#.", "...#.", "#..#.", ".##.."],
    "K": ["#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"],
    "L": ["#....", "#....", "#....", "#....", "#....", "#....", "#####"],
    "M": ["#...#", "##.##", "#.#.#", "#.#.#", "#...#", "#...#", "#...#"],
    "N": ["#...#", "#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#"],
    "O": [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
    "P": ["####.", "#...#", "#...#", "####.", "#....", "#....", "#...."],
    "Q": [".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#"],
    "R": ["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"],
    "S": [".####", "#....", "#....", ".###.", "....#", "....#", "####."],
    "T": ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."],
    "U": ["#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
    "V": ["#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#.."],
    "W": ["#...#", "#...#", "#...#", "#.#.#", "#.#.#", "#.#.#", ".#.#."],
    "X": ["#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#"],
    "Y": ["#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#.."],
    "Z": ["#####", "....#", "...#.", "..#..", ".#...", "#....", "#####"],
}
SYMBOLS = {
    "space": ["....."] * 7,
    "dash": [".....", ".....", ".....", ".###.", ".....", ".....", "....."],
    "dot": [".....", ".....", ".....", ".....", ".....", ".##..", ".##.."],
    "colon": [".....", ".##..", ".##..", ".....", ".##..", ".##..", "....."],
    "bang": ["..#..", "..#..", "..#..", "..#..", "..#..", ".....", "..#.."],
    "question": [".###.", "#...#", "....#", "...#.", "..#..", ".....", "..#.."],
    "slash": ["....#", "....#", "...#.", "..#..", ".#...", "#....", "#...."],
}
# Glyph order for ui/font/text.slint. Must match TEXT_CHARS in src/setup/mod.rs.
TEXT_CHARS = " ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-.:!?/"
TEXT_NAMES = {" ": "space", "-": "dash", ".": "dot", ":": "colon", "!": "bang", "?": "question", "/": "slash"}


# Extra punctuation for the web font only (the device screen doesn't need it).
WEB_SYMBOLS = {
    "&": [".##..", "#..#.", "#.#..", ".#...", "#.#.#", "#..#.", ".##.#"],
    ",": [".....", ".....", ".....", ".....", ".##..", "..#..", ".#..."],
    "'": ["..#..", "..#..", ".#...", ".....", ".....", ".....", "....."],
    '"': [".#.#.", ".#.#.", ".....", ".....", ".....", ".....", "....."],
    "(": ["...#.", "..#..", ".#...", ".#...", ".#...", "..#..", "...#."],
    ")": [".#...", "..#..", "...#.", "...#.", "...#.", "..#..", ".#..."],
    "+": [".....", "..#..", "..#..", "#####", "..#..", "..#..", "....."],
    "=": [".....", ".....", "#####", ".....", "#####", ".....", "....."],
    ">": [".#...", "..#..", "...#.", "....#", "...#.", "..#..", ".#..."],
    "<": ["...#.", "..#..", ".#...", "#....", ".#...", "..#..", "...#."],
    "_": [".....", ".....", ".....", ".....", ".....", ".....", "#####"],
    ";": [".....", ".##..", ".##..", ".....", ".##..", "..#..", ".#..."],
}


def text_glyph_rows(ch):
    if ch in LETTERS:
        return LETTERS[ch]
    if ch.isdigit():
        return GLYPHS[ch]
    return SYMBOLS[TEXT_NAMES[ch]]


GLYPH_INK = (255, 255, 255, 255)
GLYPH_SHADOW = (20, 24, 48, 170)


def draw_glyph(rows):
    """Glyph plus a 1px drop shadow down-right, so it reads on any sky."""
    w, h = len(rows[0]) + 1, len(rows) + 1
    px = [[CLEAR] * w for _ in range(h)]
    for y, row in enumerate(rows):
        for x, c in enumerate(row):
            if c == "#":
                px[y + 1][x + 1] = GLYPH_SHADOW
    for y, row in enumerate(rows):
        for x, c in enumerate(row):
            if c == "#":
                px[y][x] = GLYPH_INK
    return px


# ---------------------------------------------------------------- web font

def build_ttf(family="RadixPixel"):
    """A minimal TrueType font from the 5x7 glyphs: each pixel run becomes a square contour.

    1 art pixel = 100 units, 800 units per em, so at 16px / 24px / 32px every pixel lands on
    whole screen pixels and stays crisp. Lowercase maps to the uppercase glyphs.
    """
    px, upm, advance, ascent, descent = 100, 800, 600, 700, 100

    def rects(rows):
        runs = []
        for row in rows:
            found, x = [], 0
            while x < len(row):
                if row[x] == "#":
                    start = x
                    while x < len(row) and row[x] == "#":
                        x += 1
                    found.append((start, x))
                else:
                    x += 1
            runs.append(found)
        used, out = set(), []
        for r in range(len(rows)):
            for run in runs[r]:
                if (r, run) in used:
                    continue
                bottom = r
                while bottom + 1 < len(rows) and run in runs[bottom + 1]:
                    bottom += 1
                    used.add((bottom, run))
                out.append((run[0] * px, run[1] * px, (6 - bottom) * px, (7 - r) * px))
        return out

    def glyph(rect_list):
        if not rect_list:
            return b"", (0, 0, 0, 0), 0, 0
        points, ends = [], []
        for x0, x1, y0, y1 in rect_list:  # clockwise outer contours
            points += [(x0, y0), (x0, y1), (x1, y1), (x1, y0)]
            ends.append(len(points) - 1)
        xs, ys = [p[0] for p in points], [p[1] for p in points]
        bbox = (min(xs), min(ys), max(xs), max(ys))
        data = struct.pack(">hhhhh", len(ends), *bbox)
        data += b"".join(struct.pack(">H", e) for e in ends)
        data += struct.pack(">H", 0) + bytes([0x01] * len(points))
        for coords in (xs, ys):
            prev = 0
            for c in coords:
                data += struct.pack(">h", c - prev)
                prev = c
        data += b"\0" * (-len(data) % 4)
        return data, bbox, len(points), len(ends)

    chars = [c for c in TEXT_CHARS] + list(WEB_SYMBOLS)
    shapes = [[(50, 150, 0, 700)]]  # .notdef: a bar
    cmap = {}
    for ch in chars:
        rows = WEB_SYMBOLS[ch] if ch in WEB_SYMBOLS else text_glyph_rows(ch)
        cmap[ord(ch)] = len(shapes)
        if ch.isalpha():
            cmap[ord(ch.lower())] = len(shapes)
        shapes.append(rects(rows))
    cmap[0xA0] = cmap[ord(" ")]

    glyf, loca, hmtx = b"", [0], b""
    max_points = max_contours = 0
    for shape in shapes:
        data, bbox, points, contours = glyph(shape)
        glyf += data
        loca.append(len(glyf))
        hmtx += struct.pack(">Hh", advance, bbox[0])
        max_points, max_contours = max(max_points, points), max(max_contours, contours)
    n = len(shapes)

    head = struct.pack(">IIIIHHqqhhhhHHhhh", 0x00010000, 0x00010000, 0, 0x5F0F3CF5, 0x000B, upm,
                       0, 0, 0, -descent, advance, ascent, 0, 8, 2, 1, 0)
    hhea = struct.pack(">Ihhh" + "Hhhhhhh" + "hhhh" + "hH", 0x00010000, ascent, -descent, 100,
                       advance, 0, 0, advance, 1, 0, 0, 0, 0, 0, 0, 0, n)
    maxp = struct.pack(">IHHHHHHHHHHHHHH", 0x00010000, n, max_points, max_contours,
                       0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0)
    os2 = struct.pack(">HhHHH" + "hhhhhhhh" + "hhh" + "10s" + "IIII" + "4s" + "HHH" + "hhhHH" + "II" + "hhHHH",
                      4, advance, 400, 5, 0,
                      400, 400, 0, 100, 400, 400, 0, 400,
                      50, 300, 0,
                      bytes(10),
                      1, 0, 0, 0,
                      b"NONE",
                      0x0040, 32, max(c for c in cmap if c < 0x100),
                      ascent, -descent, 100, ascent, descent,
                      1, 0,
                      500, ascent, 0, 32, 0)
    post = struct.pack(">IihhIIIII", 0x00030000, 0, -100, 50, 1, 0, 0, 0, 0)

    records, strings = [], b""
    for name_id, text in ((1, family), (2, "Regular"), (3, family + "-1.0"), (4, family), (6, family + "-Regular")):
        enc = text.encode("utf-16-be")
        records.append(struct.pack(">HHHHHH", 3, 1, 0x409, name_id, len(enc), len(strings)))
        strings += enc
    name = struct.pack(">HHH", 0, len(records), 6 + 12 * len(records)) + b"".join(records) + strings

    segments = [(c, c, (g - c) & 0xFFFF) for c, g in sorted(cmap.items())] + [(0xFFFF, 0xFFFF, 1)]
    seg_count = len(segments)
    entry_selector = seg_count.bit_length() - 1
    search_range = 2 * (1 << entry_selector)
    sub = struct.pack(">HHHHHHH", 4, 16 + 8 * seg_count, 0, 2 * seg_count, search_range,
                      entry_selector, 2 * seg_count - search_range)
    sub += b"".join(struct.pack(">H", e) for _, e, _ in segments) + struct.pack(">H", 0)
    sub += b"".join(struct.pack(">H", st) for st, _, _ in segments)
    sub += b"".join(struct.pack(">H", d) for _, _, d in segments)
    sub += bytes(2 * seg_count)
    cmap_table = struct.pack(">HHHHI", 0, 1, 3, 1, 12) + sub

    tables = {
        b"OS/2": os2, b"cmap": cmap_table, b"glyf": glyf, b"head": head, b"hhea": hhea,
        b"hmtx": hmtx, b"loca": b"".join(struct.pack(">I", o) for o in loca), b"maxp": maxp,
        b"name": name, b"post": post,
    }

    def checksum(data):
        data += b"\0" * (-len(data) % 4)
        return sum(struct.unpack(f">{len(data) // 4}I", data)) & 0xFFFFFFFF

    selector = len(tables).bit_length() - 1
    directory = struct.pack(">IHHHH", 0x00010000, len(tables), 16 << selector, selector,
                            16 * len(tables) - (16 << selector))
    offset, body, head_at = 12 + 16 * len(tables), b"", 0
    for tag in sorted(tables):
        data = tables[tag]
        if tag == b"head":
            head_at = offset
        directory += struct.pack(">4sIII", tag, checksum(data), offset, len(data))
        padded = data + b"\0" * (-len(data) % 4)
        body += padded
        offset += len(padded)
    font = bytearray(directory + body)
    adjust = (0xB1B0AFBA - checksum(bytes(font))) & 0xFFFFFFFF
    font[head_at + 8:head_at + 12] = struct.pack(">I", adjust)
    return bytes(font)


# ---------------------------------------------------------------- weather icons

ICON = 12
SUN = (255, 210, 63, 255)
SUN_EDGE = (240, 138, 36, 255)
MOON = (255, 242, 176, 255)
MOON_EDGE = (200, 180, 100, 255)
CLOUD_PAL = {"W": (255, 255, 255, 255), "G": (200, 210, 224, 255), "D": (110, 122, 144, 255)}
RAIN = (74, 168, 255, 255)
SNOW = (232, 244, 255, 255)
BOLT = (255, 225, 74, 255)
CLOUD = [
    "....DDD.....",
    "...DWWWD.DD.",
    ".DDWWWWWDWWD",
    "DWWWWWWWWWWD",
    "DWWWWWWWWWGD",
    ".DGGGGGGGGD.",
    "..DDDDDDDD..",
]


def blank(size=ICON):
    return [[CLEAR] * size for _ in range(size)]


def put(px, x, y, color):
    if 0 <= y < len(px) and 0 <= x < len(px[0]):
        px[y][x] = color


def sun(px, ox=0.0, oy=0.0, r=3.0):
    c = (len(px) - 1) / 2
    cx, cy = c + ox, c + oy
    for y in range(len(px)):
        for x in range(len(px)):
            d = math.hypot(x - cx, y - cy)
            if d <= r - 0.4:
                px[y][x] = SUN
            elif d <= r + 0.5:
                px[y][x] = SUN_EDGE
    for i in range(8):
        a = i * math.pi / 4
        dist = r + 2 if i % 2 == 0 else r + 1.5
        put(px, round(cx + math.cos(a) * dist), round(cy + math.sin(a) * dist), SUN)


def moon(px, ox=0.0, oy=0.0, r=4.0, body=MOON, edge=MOON_EDGE):
    c = (len(px) - 1) / 2
    cx, cy = c + ox, c + oy
    for y in range(len(px)):
        for x in range(len(px)):
            d = math.hypot(x - cx, y - cy)
            bite = math.hypot(x - (cx + r * 0.55), y - (cy - r * 0.35))
            if d <= r + 0.3 and bite > r * 0.8:
                px[y][x] = edge if d > r - 0.7 or bite <= r * 0.8 + 0.9 else body


def cloud(px, ox=0, oy=0):
    for y, row in enumerate(CLOUD):
        for x, c in enumerate(row):
            if c != ".":
                put(px, x + ox, y + oy, CLOUD_PAL[c])


def icon_clear_day():
    px = blank()
    sun(px, r=3.3)
    return px


def icon_clear_night():
    px = blank()
    moon(px, r=4.6)
    return px


def icon_partly_day():
    px = blank()
    sun(px, -2, -2, 2.8)
    cloud(px, 0, 5)
    return px


def icon_partly_night():
    px = blank()
    moon(px, -2, -2, 3.6)
    cloud(px, 0, 5)
    return px


def icon_cloudy():
    px = blank()
    cloud(px, 0, 3)
    return px


def icon_fog():
    px = blank()
    for y, x0, x1, c in ((2, 2, 9, "D"), (4, 0, 8, "G"), (6, 3, 11, "D"), (8, 1, 9, "G"), (10, 4, 10, "D")):
        for x in range(x0, x1 + 1):
            px[y][x] = CLOUD_PAL[c]
    return px


def icon_rain():
    px = blank()
    cloud(px)
    for x, y in ((3, 8), (2, 9), (6, 9), (5, 10), (9, 8), (8, 9)):
        px[y][x] = RAIN
    return px


def icon_snow():
    px = blank()
    cloud(px)
    for cx, cy in ((3, 9), (8, 10)):
        for dx, dy in ((0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)):
            px[cy + dy][cx + dx] = SNOW
    return px


def icon_storm():
    px = blank()
    cloud(px)
    for x, y in ((6, 7), (5, 8), (4, 9), (5, 9), (6, 9), (5, 10), (4, 11)):
        px[y][x] = BOLT
    return px


# Order matters: it's the index the firmware sends (see src/weather.rs).
ICONS = [
    ("clear_day", icon_clear_day),
    ("clear_night", icon_clear_night),
    ("partly_day", icon_partly_day),
    ("partly_night", icon_partly_night),
    ("cloudy", icon_cloudy),
    ("fog", icon_fog),
    ("rain", icon_rain),
    ("snow", icon_snow),
    ("storm", icon_storm),
]


def sky_moon():
    px = blank(10)
    moon(px, r=4.4, body=(255, 246, 204, 255), edge=(214, 196, 130, 255))
    return px

# ---------------------------------------------------------------- bins

# K outline, L lid, l lid shadow, H body highlight, B body, b body shade, W wheel, w hub.
BIN = [
    "..KKKKKKKKKKKK..",
    ".KLLLLLLLLLLLLK.",
    "KLLLLLLLLLLLLLLK",
    "KllllllllllllllK",
    "KKKKKKKKKKKKKKKK",
    ".KHBBBBBBBBBBbK.",
    ".KHBBBBBBBBBBbK.",
    ".KHBBBBBBBBBBbK.",
    ".KHBBBBBBBBBBbK.",
    ".KHBBBBBBBBBBbK.",
    ".KHBBBBBBBBBBbK.",
    "..KHBBBBBBBBbK..",
    "..KHBBBBBBBBbK..",
    "..KHBBBBBBBBbK..",
    "..KHBBBBBBBBbK..",
    "..KHBBBBBBBBbK..",
    "..KbbbbbbbbbbK..",
    "..KKKKKKKKKKKK..",
    "..WwW......WwW..",
    "..WWW......WWW..",
]
BIN_FIXED = {"K": (20, 24, 48, 255), "W": (34, 34, 40, 255), "w": (130, 130, 140, 255)}
# Order matters: it's the index the firmware sends (see src/bins.rs).
# name: (lid, lid shadow, highlight, body, body shade)
BIN_COLOURS = [
    ("red", ((226, 60, 56, 255), (168, 32, 40, 255), (244, 124, 108, 255), (206, 44, 46, 255), (146, 26, 36, 255))),
    ("green", ((70, 176, 76, 255), (38, 122, 52, 255), (132, 214, 116, 255), (52, 154, 62, 255), (32, 104, 46, 255))),
    ("yellow", ((252, 214, 60, 255), (204, 160, 24, 255), (255, 238, 150, 255), (242, 198, 44, 255), (184, 140, 20, 255))),
]


def draw_bin(colours):
    palette = dict(BIN_FIXED)
    palette.update(zip("LlHBb", colours))
    return [[palette.get(c, CLEAR) for c in row] for row in BIN]


if __name__ == "__main__":
    for name, rows in GLYPHS.items():
        glyph = draw_glyph(rows)
        write_png(os.path.join(UI_DIR, "font", "big", f"{name}.png"), glyph, 4)
        write_png(os.path.join(UI_DIR, "font", "small", f"{name}.png"), glyph, 3)
    print("wrote ui/font/{big,small}/*.png")

    names = []
    for ch in TEXT_CHARS:
        name = TEXT_NAMES.get(ch, ch)
        write_png(os.path.join(UI_DIR, "font", "text", f"{name}.png"), draw_glyph(text_glyph_rows(ch)), 2)
        names.append(name)
    with open(os.path.join(UI_DIR, "font", "text.slint"), "w") as f:
        f.write("// Generated by tools/gen_sprites.py - do not edit.\n")
        f.write(f"// Glyph order: {TEXT_CHARS!r}\n")
        f.write("export global PixelFont {\n    out property <[image]> glyphs: [\n")
        for name in names:
            f.write(f'        @image-url("text/{name}.png"),\n')
        f.write("    ];\n}\n")
    print("wrote ui/font/text/*.png, ui/font/text.slint")

    os.makedirs(os.path.join(UI_DIR, "web"), exist_ok=True)
    with open(os.path.join(UI_DIR, "web", "pixel.ttf"), "wb") as f:
        f.write(build_ttf())
    print("wrote ui/web/pixel.ttf")

    for name, draw in ICONS:
        write_png(os.path.join(UI_DIR, "weather", f"{name}.png"), draw(), 3)
    print("wrote ui/weather/*.png")

    write_png(os.path.join(UI_DIR, "sky", "moon.png"), sky_moon(), 2)
    print("wrote ui/sky/moon.png")

    for name, colours in BIN_COLOURS:
        write_png(os.path.join(UI_DIR, "bins", f"{name}.png"), draw_bin(colours), 3)
    print("wrote ui/bins/*.png")
