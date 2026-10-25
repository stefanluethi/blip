"""Draws the blip icon and exports it as the app's icons.

The icon is a lowercase b drawn as one stroke, like a trace: down the stem and
around the bowl, ending in the blip. Its colour runs along the stroke through
the trace palette (blue, violet, magenta, orange, amber).

    python3 gen.py

writes the SVGs next to this script and exports the PNGs, the favicon and
icon.svg into blip/assets. It needs rsvg-convert (librsvg) and ImageMagick.
"""

import math
import subprocess
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ASSETS = HERE.parent.parent / "blip" / "assets"

# Trace palette of the app (theme.rs), in hue order along the stroke.
PALETTE = ["#648fff", "#785ef0", "#dc267f", "#fe6100", "#ffb000"]

# Geometry of the b in a 100 x 100 box: stem x, stem top, bowl centre and radius.
X, TOP, CX, CY, R = 32, 20, 50, 62, 18
STEM = CY - TOP
BOWL = R * 1.5 * math.pi  # three quarters of a circle, from the stem to the top of the bowl
LENGTH = STEM + BOWL
SEGMENTS = 180


def mix(t):
    """The palette colour at `t` (0..1) along the stroke."""
    s = min(len(PALETTE) - 1.0001, t * (len(PALETTE) - 1))
    i, f = int(s), s - int(s)
    a = [int(PALETTE[i][k : k + 2], 16) for k in (1, 3, 5)]
    b = [int(PALETTE[i + 1][k : k + 2], 16) for k in (1, 3, 5)]
    return "#%02x%02x%02x" % tuple(round(u + (v - u) * f) for u, v in zip(a, b))


def point(d):
    """The point at distance `d` along the stroke."""
    if d <= STEM:
        return X, TOP + d
    angle = math.pi - (d - STEM) / R
    return CX + R * math.cos(angle), CY + R * math.sin(angle)


def stroke(width):
    """The b as short segments, each in its colour along the stroke."""
    lines = []
    for k in range(SEGMENTS):
        (x1, y1), (x2, y2) = point(k / SEGMENTS * LENGTH), point((k + 1) / SEGMENTS * LENGTH)
        lines.append(
            f'<line x1="{x1:.2f}" y1="{y1:.2f}" x2="{x2:.2f}" y2="{y2:.2f}" '
            f'stroke="{mix(k / SEGMENTS)}" stroke-width="{width}" stroke-linecap="round"/>'
        )
    return "\n    ".join(lines)


def svg(variant):
    """standard: rounded card. maskable, ios: full bleed. small: for the favicon."""
    full = variant in ("maskable", "ios")
    small = variant == "small"
    # Maskable icons keep their content inside the central circle of 80 % diameter.
    scale = {"maskable": 0.8, "small": 1.08}.get(variant, 1.0)
    inset, radius = (0, 0) if full else (1, 20) if small else (4, 21)
    size = 100 - 2 * inset
    width = 13 if small else 11
    blip = 8.5 if small else 7.5
    glow = "" if small else f'<circle cx="{CX}" cy="{CY - R}" r="19.5" fill="url(#glow)"/>'
    border = (
        ""
        if full
        else f'<rect x="{inset + 0.4}" y="{inset + 0.4}" width="{size - 0.8}" height="{size - 0.8}" '
        f'rx="{radius - 0.4}" fill="none" stroke="#fff" stroke-opacity=".08" stroke-width=".8"/>'
    )
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="1024" height="1024">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#1e232b"/>
      <stop offset="1" stop-color="#111217"/>
    </linearGradient>
    <radialGradient id="glow">
      <stop offset="0" stop-color="{PALETTE[-1]}" stop-opacity=".75"/>
      <stop offset=".4" stop-color="{PALETTE[-1]}" stop-opacity=".3"/>
      <stop offset="1" stop-color="{PALETTE[-1]}" stop-opacity="0"/>
    </radialGradient>
  </defs>
  <rect x="{inset}" y="{inset}" width="{size}" height="{size}" rx="{radius}" fill="url(#bg)"/>
  {border}
  <g transform="translate(50 50) scale({scale}) translate(-50 -50)">
    {stroke(width)}
    {glow}
    <circle cx="{CX}" cy="{CY - R}" r="{blip}" fill="#fff"/>
  </g>
</svg>
"""


def render(source, size, target):
    subprocess.run(["rsvg-convert", "-w", str(size), "-h", str(size), source, "-o", target], check=True)


def main():
    for variant in ("standard", "maskable", "ios", "small"):
        (HERE / f"blip-{variant}.svg").write_text(svg(variant))
    (ASSETS / "icon.svg").write_text(svg("standard"))
    render(HERE / "blip-standard.svg", 1024, ASSETS / "icon-1024.png")
    render(HERE / "blip-standard.svg", 256, ASSETS / "icon-256.png")
    render(HERE / "blip-ios.svg", 192, ASSETS / "icon_ios_touch_192.png")
    render(HERE / "blip-maskable.svg", 512, ASSETS / "maskable_icon_x512.png")
    with tempfile.TemporaryDirectory() as tmp:
        sizes = [Path(tmp) / f"{s}.png" for s in (16, 32, 48)]
        for s, path in zip((16, 32, 48), sizes):
            render(HERE / "blip-small.svg", s, path)
        subprocess.run(["magick", *sizes, ASSETS / "favicon.ico"], check=True)


if __name__ == "__main__":
    main()
