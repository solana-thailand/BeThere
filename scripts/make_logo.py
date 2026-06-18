#!/usr/bin/env python3
"""
BeThere - Logo Mark Generator
Renders the BeThere check-in mark as a high-resolution PNG with transparency.

Design:
    - Rounded square with Solana diagonal gradient (#9945FF -> #14F195)
    - White circle outline (the "attendance" ring)
    - White checkmark inside (the "check-in" action)

Output:
    scripts/assets/bethere_logo.png  (600x600 RGBA)

Run:
    python3 scripts/make_logo.py

Rendered at 2x then downscaled with LANCZOS for crisp anti-aliased edges.
"""

from pathlib import Path

from PIL import Image, ImageDraw

# ---------- Brand palette (Solana) ----------
PURPLE = (0x99, 0x45, 0xFF)  # top-left
GREEN = (0x14, 0xF1, 0x95)  # bottom-right
WHITE = (0xFF, 0xFF, 0xFF)

# ---------- Geometry (final pixel units) ----------
FINAL = 600
SUPER = 2  # SSAA factor: render at 2x then downscale
RENDER = FINAL * SUPER

# Rounded square
CORNER_RADIUS = 90

# Circle (attendance ring)
CIRCLE_CENTER = (300, 270)
CIRCLE_RADIUS = 135
CIRCLE_STROKE = 16

# Checkmark (check-in action) - start -> vertex -> end
CHECK_START = (235, 272)
CHECK_VERTEX = (290, 325)
CHECK_END = (380, 215)
CHECK_STROKE = 26

OUT_PATH = Path(__file__).resolve().parent / "assets" / "bethere_logo.png"


def _lerp(color_a, color_b, t):
    """Linear interpolation between two RGB tuples at parameter t in [0, 1]."""
    return tuple(int(color_a[i] + (color_b[i] - color_a[i]) * t) for i in range(3))


def build_gradient(size: int) -> Image.Image:
    """Diagonal gradient from PURPLE (top-left) to GREEN (bottom-right).

    Uses the identity that a diagonal gradient equals the average of a
    horizontal and a vertical gradient, avoiding any per-pixel loop.
    """
    # Horizontal gradient: left=PURPLE, right=GREEN.
    row = Image.new("RGB", (size, 1))
    row.putdata([_lerp(PURPLE, GREEN, x / (size - 1)) for x in range(size)])
    horizontal = row.resize((size, size))

    # Vertical gradient: top=PURPLE, bottom=GREEN.
    col = Image.new("RGB", (1, size))
    col.putdata([_lerp(PURPLE, GREEN, y / (size - 1)) for y in range(size)])
    vertical = col.resize((size, size))

    # Diagonal = average of horizontal and vertical.
    return Image.blend(horizontal, vertical, 0.5)


def build_mask(size: int, radius: int) -> Image.Image:
    """Alpha mask: opaque inside the rounded square, transparent outside."""
    mask = Image.new("L", (size, size), 0)
    draw = ImageDraw.Draw(mask)
    draw.rounded_rectangle(
        [(0, 0), (size - 1, size - 1)],
        radius=radius,
        fill=255,
    )
    return mask


def draw_motif(logo: Image.Image, scale: int) -> None:
    """Draw the attendance ring + checkmark on the RGBA logo at render scale."""
    draw = ImageDraw.Draw(logo)
    s = scale

    # Circle outline.
    cx, cy = CIRCLE_CENTER
    r = CIRCLE_RADIUS
    sw = CIRCLE_STROKE
    bbox = [
        ((cx - r) * s, (cy - r) * s),
        ((cx + r) * s, (cy + r) * s),
    ]
    draw.ellipse(bbox, outline=WHITE, width=sw * s)

    # Checkmark polyline (rounded joints via filled cap circles).
    pts = [
        (CHECK_START[0] * s, CHECK_START[1] * s),
        (CHECK_VERTEX[0] * s, CHECK_VERTEX[1] * s),
        (CHECK_END[0] * s, CHECK_END[1] * s),
    ]
    draw.line(pts, fill=WHITE, width=CHECK_STROKE * s, joint="curve")
    cap_r = (CHECK_STROKE * s) // 2
    for px, py in pts:
        draw.ellipse(
            [(px - cap_r, py - cap_r), (px + cap_r, py + cap_r)],
            fill=WHITE,
        )


def main() -> None:
    # Render-scale gradient + rounded-square mask, composed into RGBA.
    grad = build_gradient(RENDER)
    mask = build_mask(RENDER, CORNER_RADIUS * SUPER)
    logo = Image.new("RGBA", (RENDER, RENDER), (0, 0, 0, 0))
    logo.paste(grad, (0, 0), mask)

    # Overlay the ring + checkmark at render scale.
    draw_motif(logo, SUPER)

    # Downscale with LANCZOS for anti-aliased edges.
    final = logo.resize((FINAL, FINAL), Image.Resampling.LANCZOS)

    OUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    final.save(OUT_PATH)
    size_bytes = OUT_PATH.stat().st_size
    print(f"wrote {OUT_PATH} ({final.size}, {size_bytes} bytes)")


if __name__ == "__main__":
    main()
