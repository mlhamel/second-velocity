#!/usr/bin/env python3
"""Generate placeholder art for Second Velocity: The Living Galaxy.

Designers: these PNGs are stand-ins. Replace any file in
assets/textures/ with your own; content/ stays untouched as long as
file names and sizes follow docs/gdd/art-guide.md.
"""

import os
import struct
import zlib

SIZE = 128
SHIP_COLOR = (235, 235, 235, 255)
COCKPIT_COLOR = (70, 70, 70, 255)
ENGINE_COLOR = (255, 170, 60, 255)


def write_png(path, width, height, pixels):
    raw = b"".join(b"\x00" + pixels[y * width * 4 : (y + 1) * width * 4] for y in range(height))
    def chunk(tag, data):
        block = tag + data
        return struct.pack(">I", len(data)) + block + struct.pack(">I", zlib.crc32(block))
    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9))
    png += chunk(b"IEND", b"")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as handle:
        handle.write(png)
    print(f"wrote {path}")


def blend(buffer, x, y, color, alpha):
    index = (int(y) * SIZE + int(x)) * 4
    for channel in range(4):
        old = buffer[index + channel]
        new = color[channel] if channel < 3 else 255
        buffer[index + channel] = int(old * (1 - alpha) + new * alpha)


def fill_polygon(buffer, points, color):
    scaled = [(x * SIZE, y * SIZE) for x, y in points]
    for y in range(SIZE):
        for x in range(SIZE):
            coverage = 0.0
            for dx in (0.25, 0.75):
                for dy in (0.25, 0.75):
                    px, py = x + dx, y + dy
                    inside = False
                    j = len(scaled) - 1
                    for i in range(len(scaled)):
                        xi, yi = scaled[i]
                        xj, yj = scaled[j]
                        if (yi > py) != (yj > py) and px < (xj - xi) * (py - yi) / (yj - yi) + xi:
                            inside = not inside
                        j = i
                    coverage += 0.25 if inside else 0.0
            if coverage > 0.0:
                blend(buffer, x, y, color, coverage)


def fill_circle(buffer, cx, cy, radius, color, falloff=1.0):
    for y in range(SIZE):
        for x in range(SIZE):
            distance = ((x - cx) ** 2 + (y - cy) ** 2) ** 0.5
            if distance <= radius:
                alpha = (1.0 - (distance / radius) ** falloff) if falloff != 0 else 1.0
                blend(buffer, x, y, color, min(1.0, max(0.0, alpha)))


def ship(buffer, hull_color=SHIP_COLOR):
    hull = [(0.96, 0.50), (0.52, 0.36), (0.14, 0.40), (0.14, 0.60), (0.52, 0.64)]
    wing_top = [(0.55, 0.37), (0.22, 0.06), (0.10, 0.10), (0.30, 0.42)]
    wing_bottom = [(0.55, 0.63), (0.22, 0.94), (0.10, 0.90), (0.30, 0.58)]
    engine = [(0.06, 0.44), (0.14, 0.44), (0.14, 0.56), (0.06, 0.56)]
    fill_polygon(buffer, wing_top, hull_color)
    fill_polygon(buffer, wing_bottom, hull_color)
    fill_polygon(buffer, hull, hull_color)
    fill_polygon(buffer, engine, ENGINE_COLOR)
    fill_circle(buffer, SIZE * 0.62, SIZE * 0.5, SIZE * 0.055, COCKPIT_COLOR, 0)


def planet(buffer, base):
    cx = cy = SIZE / 2
    radius = SIZE * 0.44
    for y in range(SIZE):
        for x in range(SIZE):
            distance = ((x - cx) ** 2 + (y - cy) ** 2) ** 0.5
            if distance <= radius:
                shade = 0.45 + 0.55 * (1 - (distance / radius) ** 2) ** 0.5
                color = tuple(int(channel * shade) for channel in base) + (255,)
                blend(buffer, x, y, color, 1.0)
            elif distance <= radius * 1.12:
                alpha = 1.0 - (distance - radius) / (radius * 0.12)
                color = (base[0], base[1], base[2], 255)
                blend(buffer, x, y, color, alpha * 0.35)


def star(buffer):
    cx = cy = SIZE / 2
    disc = SIZE * 0.30
    corona = SIZE * 0.48
    fill_circle(buffer, cx, cy, corona, (255, 120, 40, 255), 3.0)
    fill_circle(buffer, cx, cy, corona * 0.8, (255, 160, 60, 255), 2.6)
    fill_circle(buffer, cx, cy, corona * 0.62, (255, 200, 110, 255), 2.2)
    fill_circle(buffer, cx, cy, disc, (255, 244, 214, 255), 0)
    for y in range(SIZE):
        for x in range(SIZE):
            distance = ((x - cx) ** 2 + (y - cy) ** 2) ** 0.5
            if disc * 0.82 <= distance <= disc:
                blend(buffer, x, y, (250, 190, 120, 255), 0.55)


def black_hole(buffer):
    import math

    for y in range(SIZE):
        for x in range(SIZE):
            dx = (x + 0.5) / SIZE - 0.5
            dy = (y + 0.5) / SIZE - 0.5
            distance = math.hypot(dx, dy)
            index = (y * SIZE + x) * 4
            if distance <= 0.25:
                buffer[index:index + 4] = [2, 1, 6, 255]
            elif distance <= 0.48:
                ring = max(0.0, 1.0 - abs((distance - 0.32) / 0.16))
                brightness = 0.65 + 0.35 * math.sin(math.atan2(dy, dx) * 3.0)
                buffer[index:index + 4] = [
                    int(210 * brightness), int(110 * brightness), 245, int(ring * 255)
                ]


def chief(buffer, coat_color, accent_color):
    # Framed portrait backdrop disc
    fill_circle(buffer, SIZE * 0.5, SIZE * 0.52, SIZE * 0.44, (24, 30, 42, 255), 0)
    # Shoulders / uniform coat
    shoulders = [
        (0.18, 0.92),
        (0.24, 0.64),
        (0.40, 0.56),
        (0.60, 0.56),
        (0.76, 0.64),
        (0.82, 0.92),
    ]
    collar = [
        (0.38, 0.56),
        (0.50, 0.72),
        (0.62, 0.56),
    ]
    fill_polygon(buffer, shoulders, coat_color + (255,))
    fill_polygon(buffer, collar, accent_color + (255,))
    # Head & visor/eyes
    fill_circle(buffer, SIZE * 0.5, SIZE * 0.38, SIZE * 0.16, (215, 190, 168, 255), 0)
    visor = [(0.38, 0.34), (0.62, 0.34), (0.60, 0.40), (0.40, 0.40)]
    fill_polygon(buffer, visor, accent_color + (255,))


def write_if_missing(path, width, height, pixels, force=False):
    if os.path.exists(path) and not force:
        print(f"kept existing {path} (pass --force to overwrite)")
        return
    write_png(path, width, height, pixels)


def main():
    import sys

    force = "--force" in sys.argv[1:]
    root = os.path.join(os.path.dirname(__file__), "..", "assets", "textures")

    ship_pixels = [0] * (SIZE * SIZE * 4)
    ship(ship_pixels)
    for ship_name in ("shuttle.png", "turncoat.png", "patrol.png"):
        write_if_missing(os.path.join(root, "ships", ship_name), SIZE, SIZE, bytes(ship_pixels), force)

    for ship_name, hull_color in [
        ("terran-courier.png", (200, 210, 225, 255)),
        ("terran-frigate.png", (150, 165, 190, 255)),
        ("vellari-sprinter.png", (240, 225, 180, 255)),
        ("korath-cutter.png", (140, 120, 115, 255)),
        ("korath-reaver.png", (120, 100, 100, 255)),
        ("sylph-skiff.png", (170, 220, 205, 255)),
        ("sylph-dreamweaver.png", (140, 200, 180, 255)),
        ("auroran-titan.png", (200, 130, 70, 255)),
    ]:
        tinted = [0] * (SIZE * SIZE * 4)
        ship(tinted, hull_color)
        write_if_missing(os.path.join(root, "ships", ship_name), SIZE, SIZE, bytes(tinted), force)

    for name, base in [
        ("earth.png", (46, 96, 168)),
        ("newhaven.png", (150, 118, 88)),
        ("kestrel-drift.png", (128, 108, 82)),
        ("greenreach.png", (92, 140, 78)),
        ("deepwatch.png", (96, 90, 130)),
        ("hawking-stand.png", (110, 105, 120)),
    ]:
        planet_pixels = [0] * (SIZE * SIZE * 4)
        planet(planet_pixels, base)
        write_if_missing(os.path.join(root, "planets", name), SIZE, SIZE, bytes(planet_pixels), force)

    star_pixels = [0] * (SIZE * SIZE * 4)
    star(star_pixels)
    write_if_missing(os.path.join(root, "stars", "corona.png"), SIZE, SIZE, bytes(star_pixels), force)

    hole_pixels = [0] * (SIZE * SIZE * 4)
    black_hole(hole_pixels)
    write_if_missing(os.path.join(root, "black-holes", "accretion.png"), SIZE, SIZE, bytes(hole_pixels), force)

    for name, coat, accent in [
        ("earth.png", (48, 88, 150), (210, 175, 75)),
        ("newhaven.png", (128, 92, 62), (90, 190, 175)),
        ("kestrel-drift.png", (110, 78, 52), (230, 150, 60)),
        ("greenreach.png", (74, 118, 66), (200, 180, 90)),
        ("deepwatch.png", (58, 56, 96), (120, 190, 220)),
        ("hawking-stand.png", (90, 88, 96), (220, 120, 110)),
        ("sirius-prime.png", (70, 60, 92), (230, 200, 90)),
        ("vega-station.png", (70, 90, 110), (90, 200, 140)),
        ("tarsis-port.png", (100, 80, 70), (180, 90, 160)),
        ("kashmir-fields.png", (120, 100, 60), (230, 140, 90)),
    ]:
        chief_pixels = [0] * (SIZE * SIZE * 4)
        chief(chief_pixels, coat, accent)
        write_if_missing(os.path.join(root, "chiefs", name), SIZE, SIZE, bytes(chief_pixels), force)


if __name__ == "__main__":
    main()