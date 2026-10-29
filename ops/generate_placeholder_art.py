#!/usr/bin/env python3
"""Pixel-art placeholder generator v2 for Second Velocity: The Living Galaxy.

Designers: these PNGs are stand-ins. Replace any file in assets/textures/
with your own; content/ stays untouched as long as file names, canvas sizes,
and content footprints follow docs/gdd/art-guide.md and ADR-0016.

Ships are read from content/ships/*.ron (id, race, size), drawn at their
native world footprint (1 texel = 1 world pixel) inside a 128x128 canvas:
Light 36, Medium 48, Heavy 68, Capital 92. Planets are 180x180 (PLANET_RADIUS
90). Everything uses nearest-friendly pixel art: limited ramps, 1px
outlines, Bayer-dithered shading, stepped glow bands.
"""

import math
import os
import re
import struct
import sys
import zlib

SHIP_CANVAS = 128
PLANET_CANVAS = 180
FOOTPRINT = {"Light": 36, "Medium": 48, "Heavy": 68, "Capital": 92}

BAYER4 = [
    [0, 8, 2, 10],
    [12, 4, 14, 6],
    [3, 11, 1, 9],
    [15, 7, 13, 5],
]


def bayer(x, y):
    return BAYER4[y % 4][x % 4] / 16.0


def clamp(value, lo, hi):
    return max(lo, min(hi, value))


def fnv1a(text):
    h = 0x811C9DC5
    for b in text.encode():
        h = ((h ^ b) * 0x01000193) & 0xFFFFFFFF
    return h


def make_rng(seed):
    state = seed | 1

    def rand():
        nonlocal state
        state = (state * 1664525 + 1013904223) & 0xFFFFFFFF
        return state / 4294967296.0

    return rand


def write_png(path, width, height, pixels):
    raw = b"".join(
        b"\x00" + pixels[y * width * 4 : (y + 1) * width * 4] for y in range(height)
    )

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


class Canvas:
    def __init__(self, size):
        self.size = size
        self.pixels = bytearray(size * size * 4)
        self.shade = [-1] * (size * size)

    def index(self, x, y):
        return (y * self.size + x) * 4

    def set(self, x, y, color, alpha=255, shade=-1):
        x, y = int(x), int(y)
        if 0 <= x < self.size and 0 <= y < self.size:
            i = self.index(x, y)
            self.pixels[i : i + 3] = bytes(color[:3])
            self.pixels[i + 3] = int(alpha)
            self.shade[y * self.size + x] = shade

    def recolor(self, x, y, color):
        x, y = int(x), int(y)
        if 0 <= x < self.size and 0 <= y < self.size:
            i = self.index(x, y)
            if self.pixels[i + 3] > 0:
                self.pixels[i : i + 3] = bytes(color[:3])

    def blend(self, x, y, color, alpha):
        x, y = int(x), int(y)
        if 0 <= x < self.size and 0 <= y < self.size:
            i = self.index(x, y)
            for channel in range(3):
                old = self.pixels[i + channel]
                self.pixels[i + channel] = int(old * (1 - alpha) + color[channel] * alpha)
            self.pixels[i + 3] = max(self.pixels[i + 3], int(alpha * 255))

    def fill_polygon(self, points, color, shade_idx=-1):
        xs = [p[0] for p in points]
        ys = [p[1] for p in points]
        x0, x1 = max(0, int(min(xs))), min(self.size - 1, int(max(xs)))
        y0, y1 = max(0, int(min(ys))), min(self.size - 1, int(max(ys)))
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                if point_in_polygon(x + 0.5, y + 0.5, points):
                    self.set(x, y, color, 255, shade_idx)

    def fill_ellipse(self, cx, cy, rx, ry, color, shade_idx=-1):
        for y in range(self.size):
            for x in range(self.size):
                if ((x + 0.5 - cx) / rx) ** 2 + ((y + 0.5 - cy) / ry) ** 2 <= 1.0:
                    self.set(x, y, color, 255, shade_idx)

    def apply_light(self, ramp, box=None, max_shift=2):
        ox, oy, w, h = box or (0, 0, self.size, self.size)
        for y in range(self.size):
            for x in range(self.size):
                s = self.shade[y * self.size + x]
                if s < 0:
                    continue
                nx = (x - (ox + w / 2)) / max(1.0, w / 2)
                ny = (y - (oy + h / 2)) / max(1.0, h / 2)
                f = clamp(0.55 + 0.4 * nx - 0.55 * ny, 0.0, 1.0)
                shift = int((1.0 - f) * max_shift + bayer(x, y))
                idx = clamp(s - shift, 0, len(ramp) - 1)
                i = self.index(x, y)
                self.pixels[i : i + 3] = bytes(ramp[idx])

    def outline(self, color):
        solid = [
            self.pixels[(y * self.size + x) * 4 + 3] > 0
            for y in range(self.size)
            for x in range(self.size)
        ]
        s = self.size
        for y in range(s):
            for x in range(s):
                if not solid[y * s + x]:
                    continue
                edge = (
                    x == 0
                    or y == 0
                    or x + 1 == s
                    or y + 1 == s
                    or not solid[y * s + x - 1]
                    or not solid[y * s + x + 1]
                    or not solid[(y - 1) * s + x]
                    or not solid[(y + 1) * s + x]
                )
                if edge:
                    i = self.index(x, y)
                    self.pixels[i : i + 3] = bytes(color)


def point_in_polygon(px, py, vertices):
    inside = False
    j = len(vertices) - 1
    for i in range(len(vertices)):
        xi, yi = vertices[i]
        xj, yj = vertices[j]
        if (yi > py) != (yj > py) and px < (xj - xi) * (py - yi) / (yj - yi) + xi:
            inside = not inside
        j = i
    return inside


def outline_color(ramp):
    return tuple(int(c * 0.55) for c in ramp[0])


def scale_color(base, factor):
    return tuple(clamp(int(c * factor), 0, 255) for c in base)


RAMPS = {
    "terran": [(46, 58, 82), (74, 92, 124), (104, 128, 166), (142, 168, 205), (186, 206, 231), (222, 236, 248)],
    "vellari": [(66, 50, 24), (96, 74, 36), (140, 108, 52), (186, 150, 80), (224, 194, 120), (246, 228, 168)],
    "sylph": [(14, 42, 38), (22, 66, 60), (34, 96, 86), (52, 134, 118), (88, 172, 152), (150, 216, 196)],
    "korath": [(40, 22, 18), (66, 38, 28), (98, 58, 40), (136, 84, 56), (176, 116, 80), (212, 156, 110)],
    "auroran": [(48, 24, 10), (78, 40, 16), (118, 64, 26), (160, 92, 40), (198, 128, 62), (236, 172, 104)],
}

METAL_RAMP = [(38, 44, 58), (70, 80, 100), (110, 124, 150), (150, 168, 192), (196, 210, 228)]

SILHOUETTES = {
    "terran": [
        ([(0.04, 0.42), (0.30, 0.30), (0.72, 0.34), (0.98, 0.50), (0.72, 0.66), (0.30, 0.70), (0.04, 0.58)], 3),
        ([(0.28, 0.33), (0.10, 0.08), (0.02, 0.13), (0.18, 0.45)], 2),
        ([(0.28, 0.67), (0.10, 0.92), (0.02, 0.87), (0.18, 0.55)], 2),
        ([(0.00, 0.40), (0.10, 0.40), (0.10, 0.60), (0.00, 0.60)], 1),
        ([(0.40, 0.38), (0.58, 0.38), (0.58, 0.62), (0.40, 0.62)], 4),
    ],
    "vellari": [
        ([(0.02, 0.40), (0.28, 0.35), (0.90, 0.46), (0.98, 0.50), (0.90, 0.54), (0.28, 0.65), (0.02, 0.60)], 3),
        ([(0.30, 0.38), (0.08, 0.05), (0.00, 0.11), (0.16, 0.44)], 2),
        ([(0.30, 0.62), (0.08, 0.95), (0.00, 0.89), (0.16, 0.56)], 2),
        ([(0.00, 0.43), (0.09, 0.43), (0.09, 0.57), (0.00, 0.57)], 1),
        ([(0.35, 0.47), (0.80, 0.47), (0.80, 0.53), (0.35, 0.53)], 4),
    ],
    "sylph": [
        ([(0.08, 0.50), (0.16, 0.32), (0.45, 0.20), (0.78, 0.28), (0.98, 0.50), (0.78, 0.72), (0.45, 0.80), (0.16, 0.68)], 3),
        ([(0.40, 0.24), (0.26, 0.02), (0.18, 0.08), (0.30, 0.34)], 2),
        ([(0.40, 0.76), (0.26, 0.98), (0.18, 0.92), (0.30, 0.66)], 2),
        ([(0.00, 0.42), (0.08, 0.42), (0.08, 0.58), (0.00, 0.58)], 1),
    ],
    "korath": [
        ([(0.02, 0.32), (0.82, 0.28), (0.98, 0.44), (0.98, 0.56), (0.82, 0.72), (0.02, 0.68)], 3),
        ([(0.16, 0.06), (0.38, 0.06), (0.38, 0.28), (0.16, 0.28)], 2),
        ([(0.16, 0.72), (0.38, 0.72), (0.38, 0.94), (0.16, 0.94)], 2),
        ([(0.00, 0.34), (0.06, 0.34), (0.06, 0.66), (0.00, 0.66)], 1),
        ([(0.60, 0.40), (0.88, 0.40), (0.88, 0.60), (0.60, 0.60)], 4),
    ],
    "auroran": [
        ([(0.02, 0.50), (0.18, 0.24), (0.55, 0.14), (0.94, 0.24), (0.98, 0.38), (0.72, 0.46), (0.98, 0.62), (0.94, 0.76), (0.55, 0.86), (0.18, 0.76)], 3),
        ([(0.30, 0.42), (0.70, 0.42), (0.70, 0.58), (0.30, 0.58)], 4),
        ([(0.00, 0.44), (0.06, 0.44), (0.06, 0.56), (0.00, 0.56)], 1),
    ],
}


def draw_ship(race, size_px, seed):
    canvas = Canvas(SHIP_CANVAS)
    ramp = RAMPS[race]
    ox = SHIP_CANVAS // 2 - size_px // 2
    oy = ox
    for points, shade_idx in SILHOUETTES[race]:
        scaled = [(ox + u * size_px, oy + v * size_px) for u, v in points]
        canvas.fill_polygon(scaled, ramp[shade_idx], shade_idx)
    canvas.apply_light(ramp, box=(ox, oy, size_px, size_px))
    canvas.outline(outline_color(ramp))

    rand = make_rng(seed)
    mid = oy + size_px // 2
    glass = (140, 216, 235)
    glint = (214, 244, 252)
    cockpit_x = ox + int(size_px * 0.62)
    for dx in range(2):
        for dy in range(2):
            canvas.recolor(cockpit_x + dx, mid - 1 + dy, glass)
    canvas.recolor(cockpit_x + 1, mid - 1, glint)

    exhaust_x = ox + max(1, int(size_px * 0.02))
    flame = (255, 170, 60)
    flame_hot = (255, 215, 120)
    for dy in (-1, 0, 1):
        canvas.set(exhaust_x - 1, mid + dy, flame)
    canvas.set(exhaust_x, mid, flame_hot)

    for _ in range(3):
        u = 0.22 + rand() * 0.55
        seam_x = ox + int(u * size_px)
        for v in range(int(size_px * 0.34), int(size_px * 0.66)):
            canvas.recolor(seam_x, oy + v, ramp[1])
    return canvas


def planet_ramp(base):
    return [scale_color(base, 0.32), scale_color(base, 0.55), scale_color(base, 0.8), tuple(base), scale_color(base, 1.3)]


def draw_planet(name, base):
    size = PLANET_CANVAS
    canvas = Canvas(size)
    cx = cy = size // 2
    radius = size // 2 - 4
    ramp = planet_ramp(base)
    rand = make_rng(fnv1a(name))
    blobs = [
        (cx + (rand() - 0.5) * radius * 1.1, cy + (rand() - 0.5) * radius * 1.1, radius * (0.18 + rand() * 0.3))
        for _ in range(4)
    ]
    band = 0.15 + rand() * 0.7
    for y in range(size):
        for x in range(size):
            d = ((x - cx) ** 2 + (y - cy) ** 2) ** 0.5
            if d <= radius:
                nx = (x - cx) / radius
                ny = (y - cy) / radius
                f = clamp(0.6 + 0.45 * nx - 0.6 * ny - 0.25 * d / radius, 0.0, 1.0)
                if d > radius * 0.9:
                    f += 0.12
                if abs(ny - (band - 0.5) * 1.6) < 0.09 and bayer(x, y) > abs(ny - (band - 0.5) * 1.6) / 0.09:
                    f -= 0.14
                for bx, by, br in blobs:
                    if (x - bx) ** 2 + (y - by) ** 2 < br * br:
                        f -= 0.18
                        break
                idx = clamp(int(f * (len(ramp) - 1) + bayer(x, y)), 0, len(ramp) - 1)
                canvas.set(x, y, ramp[idx])
            elif d <= radius + 4:
                a = 1 - (d - radius) / 4
                if bayer(x, y) < a:
                    canvas.set(x, y, (160, 190, 230), 60)
    return canvas


def draw_station():
    canvas = Canvas(SHIP_CANVAS)
    center = SHIP_CANVAS // 2
    outer, inner, strut, core = 44, 33, 3, 11
    rand = make_rng(0x5EED_5EED)
    for y in range(SHIP_CANVAS):
        for x in range(SHIP_CANVAS):
            dx = x + 0.5 - center
            dy = y + 0.5 - center
            d = (dx * dx + dy * dy) ** 0.5
            shade_idx = -1
            if d <= core:
                shade_idx = 4
            elif inner <= d <= outer:
                shade_idx = 3
                if d > outer - 2 or d < inner + 2:
                    shade_idx = 1
            if shade_idx < 0 and d <= outer and (abs(dx) <= strut or abs(dy) <= strut):
                shade_idx = 2
            if shade_idx >= 0:
                canvas.set(x, y, METAL_RAMP[shade_idx], 255, shade_idx)
    canvas.apply_light(METAL_RAMP)
    canvas.outline(outline_color(METAL_RAMP))
    for k in range(12):
        angle = k / 12 * 2 * math.pi + rand() * 0.3
        if rand() < 0.3:
            continue
        ring_r = (inner + outer) / 2
        wx = center + ring_r * math.cos(angle)
        wy = center + ring_r * math.sin(angle)
        window = (120, 240, 230) if k % 2 == 0 else (255, 220, 120)
        canvas.recolor(wx, wy, window)
        canvas.recolor(wx + 1, wy, window)
    canvas.fill_ellipse(center, center, core - 3, core - 3, (95, 230, 220))
    canvas.recolor(center, center - 4, (190, 255, 245))
    return canvas


def draw_star():
    size = SHIP_CANVAS
    canvas = Canvas(size)
    s = float(size)
    disc = s * 0.30
    corona = s * 0.48
    core = (255, 244, 214)
    bands = [
        (255, 196, 110, 235),
        (255, 140, 60, 160),
        (255, 96, 46, 95),
        (210, 60, 40, 45),
    ]
    for y in range(size):
        for x in range(size):
            dx = x + 0.5 - s / 2
            dy = y + 0.5 - s / 2
            distance = (dx * dx + dy * dy) ** 0.5
            if distance <= disc:
                canvas.set(x, y, core)
            elif distance <= corona:
                t = 1 - (distance - disc) / (corona - disc)
                level = clamp(int(t * 4 + bayer(x, y)), 0, 4)
                if level > 0:
                    canvas.set(x, y, bands[level - 1][:3], bands[level - 1][3])
    return canvas


def draw_black_hole():
    size = SHIP_CANVAS
    canvas = Canvas(size)
    s = float(size)
    rings = [
        (210, 110, 245, 235),
        (150, 70, 200, 170),
        (85, 35, 130, 100),
    ]
    for y in range(size):
        for x in range(size):
            dx = (x + 0.5) / s - 0.5
            dy = (y + 0.5) / s - 0.5
            distance = math.hypot(dx, dy)
            if distance <= 0.25:
                canvas.set(x, y, (2, 1, 6))
            elif distance <= 0.48:
                ring = 1 - abs((distance - 0.32) / 0.16)
                if ring > 0:
                    level = clamp(int(ring * 3 + bayer(x, y)), 0, 2)
                    shimmer = 0.7 + 0.3 * math.sin(math.atan2(dy, dx) * 3)
                    band = rings[level]
                    canvas.set(x, y, band[:3], int(band[3] * shimmer))
    return canvas


def draw_chief(coat, accent):
    canvas = Canvas(SHIP_CANVAS)
    size = SHIP_CANVAS
    coat = tuple(coat)
    accent = tuple(accent)
    skin = (215, 190, 168)
    hair = scale_color(coat, 0.4)
    coat_light = scale_color(coat, 1.35)
    coat_dark = scale_color(coat, 0.55)

    canvas.fill_ellipse(size * 0.5, size * 0.52, size * 0.44, size * 0.44, (24, 30, 42))
    shoulders = [(0.18, 0.92), (0.24, 0.64), (0.40, 0.56), (0.60, 0.56), (0.76, 0.64), (0.82, 0.92)]
    canvas.fill_polygon([(x * size, y * size) for x, y in shoulders], coat)
    collar = [(0.38, 0.56), (0.50, 0.72), (0.62, 0.56)]
    canvas.fill_polygon([(x * size, y * size) for x, y in collar], accent)
    for px in range(int(size * 0.20), int(size * 0.34)):
        canvas.recolor(px, int(size * 0.80), coat_light)
        canvas.recolor(px, int(size * 0.86), coat_light)
    for px in range(int(size * 0.68), int(size * 0.82)):
        canvas.recolor(px, int(size * 0.80), coat_dark)
        canvas.recolor(px, int(size * 0.86), coat_dark)
    canvas.fill_ellipse(size * 0.5, size * 0.38, size * 0.16, size * 0.17, skin)
    canvas.fill_polygon([(x * size, y * size) for x, y in [(0.34, 0.30), (0.66, 0.30), (0.62, 0.24), (0.38, 0.24)]], hair)
    visor = [(0.38, 0.34), (0.62, 0.34), (0.60, 0.40), (0.40, 0.40)]
    canvas.fill_polygon([(x * size, y * size) for x, y in visor], accent)
    canvas.recolor(size * 0.58, size * 0.36, (240, 250, 255))
    canvas.outline((12, 14, 20))
    return canvas


def draw_commodity(body, band):
    canvas = Canvas(SHIP_CANVAS)
    size = SHIP_CANVAS
    body = tuple(body)
    band = tuple(band)
    box = [(0.24, 0.26), (0.76, 0.26), (0.76, 0.74), (0.24, 0.74)]
    lid = [(0.20, 0.22), (0.80, 0.22), (0.80, 0.32), (0.20, 0.32)]
    canvas.fill_polygon([(x * size, y * size) for x, y in box], body)
    canvas.fill_polygon([(x * size, y * size) for x, y in lid], band)
    for y0 in (0.42, 0.54, 0.66):
        rib = [(0.24, y0), (0.76, y0), (0.76, y0 + 0.05), (0.24, y0 + 0.05)]
        canvas.fill_polygon([(x * size, y * size) for x, y in rib], scale_color(band, 0.7))
    canvas.outline(outline_color([body]))
    canvas.recolor(size * 0.30, size * 0.30, scale_color(body, 1.5))
    return canvas


def draw_weapon(body, tip):
    canvas = Canvas(SHIP_CANVAS)
    size = SHIP_CANVAS
    body = tuple(body)
    tip = tuple(tip)
    block = [(0.30, 0.42), (0.70, 0.42), (0.70, 0.58), (0.30, 0.58)]
    barrel = [(0.70, 0.46), (0.88, 0.46), (0.88, 0.54), (0.70, 0.54)]
    grip = [(0.38, 0.58), (0.50, 0.58), (0.46, 0.74), (0.34, 0.74)]
    for points in (block, barrel, grip):
        canvas.fill_polygon([(x * size, y * size) for x, y in points], body)
    canvas.outline(outline_color([body]))
    canvas.fill_ellipse(size * 0.90, size * 0.50, size * 0.05, size * 0.05, tip)
    canvas.recolor(size * 0.34, size * 0.45, scale_color(body, 1.5))
    return canvas


def draw_shield_generator(ring, core):
    canvas = Canvas(SHIP_CANVAS)
    size = SHIP_CANVAS
    ring = tuple(ring)
    core = tuple(core)
    for y in range(size):
        for x in range(size):
            d = ((x + 0.5 - size * 0.5) ** 2 + (y + 0.5 - size * 0.48) ** 2) ** 0.5
            if d <= size * 0.34 and d >= size * 0.22:
                level = clamp(int((1 - abs(d - size * 0.28) / (size * 0.06)) * 3 + bayer(x, y)), 0, 2)
                canvas.set(x, y, scale_color(ring, 0.5 + level * 0.25))
            elif d < size * 0.22:
                canvas.set(x, y, core)
    pedestal = [(0.36, 0.78), (0.64, 0.78), (0.64, 0.86), (0.36, 0.86)]
    canvas.fill_polygon([(x * size, y * size) for x, y in pedestal], ring)
    canvas.outline(outline_color([ring]))
    canvas.recolor(size * 0.44, size * 0.42, scale_color(core, 1.3))
    return canvas


def draw_fuel_tank():
    canvas = Canvas(SHIP_CANVAS)
    size = SHIP_CANVAS
    body = (200, 150, 70)
    cap = (150, 110, 50)
    label = (245, 230, 200)
    tank = [(0.30, 0.30), (0.70, 0.30), (0.70, 0.72), (0.30, 0.72)]
    cap_poly = [(0.42, 0.22), (0.58, 0.22), (0.58, 0.30), (0.42, 0.30)]
    canvas.fill_polygon([(x * size, y * size) for x, y in tank], body)
    canvas.fill_polygon([(x * size, y * size) for x, y in cap_poly], cap)
    canvas.outline(outline_color([body]))
    canvas.fill_ellipse(size * 0.5, size * 0.52, size * 0.07, size * 0.07, label)
    canvas.recolor(size * 0.34, size * 0.34, scale_color(body, 1.4))
    return canvas


def parse_ships(content_dir):
    ships = []
    for name in sorted(os.listdir(content_dir)):
        if not name.endswith(".ron"):
            continue
        with open(os.path.join(content_dir, name), encoding="utf-8") as handle:
            text = handle.read()
        ship_id = re.search(r'id:\s*"([^"]+)"', text)
        race = re.search(r'race:\s*"([^"]+)"', text)
        size = re.search(r"size:\s*(Light|Medium|Heavy|Capital)", text)
        if ship_id and race and size:
            ships.append((ship_id.group(1), race.group(1), size.group(1)))
    return ships


def write_if_missing(path, canvas, force=False):
    if os.path.exists(path) and not force:
        print(f"kept existing {path} (pass --force to overwrite)")
        return
    write_png(path, canvas.size, canvas.size, bytes(canvas.pixels))


def main():
    force = "--force" in sys.argv[1:]
    here = os.path.dirname(os.path.abspath(__file__))
    root = os.path.join(here, "..", "assets", "textures")
    content_dir = os.path.join(here, "..", "content")

    for ship_id, race, size in parse_ships(os.path.join(content_dir, "ships")):
        canvas = draw_ship(race if race in RAMPS else "terran", FOOTPRINT[size], fnv1a(ship_id))
        write_if_missing(os.path.join(root, "ships", f"{ship_id}.png"), canvas, force)

    for name, base in [
        ("earth.png", (46, 96, 168)),
        ("newhaven.png", (150, 118, 88)),
        ("kestrel-drift.png", (128, 108, 82)),
        ("greenreach.png", (92, 140, 78)),
        ("deepwatch.png", (96, 90, 130)),
        ("hawking-stand.png", (110, 105, 120)),
    ]:
        canvas = draw_planet(name[:-4], base)
        write_if_missing(os.path.join(root, "planets", name), canvas, force)

    write_if_missing(os.path.join(root, "stations", "docking-ring.png"), draw_station(), force)
    write_if_missing(os.path.join(root, "stars", "corona.png"), draw_star(), force)
    write_if_missing(os.path.join(root, "black-holes", "accretion.png"), draw_black_hole(), force)

    for name, coat, accent in [
        ("lin-relay.png", (50, 75, 120), (95, 230, 220)),
        ("wayfarer-exchange.png", (100, 75, 65), (230, 180, 80)),
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
        canvas = draw_chief(coat, accent)
        write_if_missing(os.path.join(root, "chiefs", name), canvas, force)

    for name, body, band in [
        ("food.png", (92, 160, 70), (60, 110, 48)),
        ("isotopes.png", (170, 200, 60), (110, 140, 40)),
        ("luxury.png", (210, 170, 70), (150, 115, 45)),
        ("machinery.png", (120, 140, 165), (80, 100, 125)),
        ("medicine.png", (215, 225, 230), (200, 60, 70)),
        ("ore.png", (140, 110, 90), (95, 75, 60)),
        ("textiles.png", (150, 110, 180), (105, 75, 135)),
        ("weapons.png", (120, 60, 60), (220, 120, 50)),
    ]:
        canvas = draw_commodity(body, band)
        write_if_missing(os.path.join(root, "commodities", name), canvas, force)

    for name, body, tip in [
        ("light-blaster.png", (170, 190, 200), (95, 230, 220)),
        ("pulse-blaster.png", (150, 140, 190), (230, 120, 235)),
        ("quad-turret.png", (130, 175, 195), (245, 200, 90)),
        ("seeker-rack.png", (195, 115, 95), (250, 225, 110)),
    ]:
        canvas = draw_weapon(body, tip)
        write_if_missing(os.path.join(root, "outfits", name), canvas, force)

    for name, ring, core in [
        ("shield-generator-1.png", (90, 200, 185), (150, 235, 225)),
        ("shield-generator-2.png", (80, 160, 235), (150, 200, 250)),
        ("capacitor-bank.png", (230, 185, 75), (250, 235, 140)),
    ]:
        canvas = draw_shield_generator(ring, core)
        write_if_missing(os.path.join(root, "outfits", name), canvas, force)

    write_if_missing(os.path.join(root, "outfits", "aux-fuel-tank.png"), draw_fuel_tank(), force)


if __name__ == "__main__":
    main()