"""The colours of blip (blip/src/app/theme.rs) for the draw.io diagrams.

The diagrams are drawn in the light theme with these colours only. `dark` maps each
light colour to the one of the dark theme, so the dark variant of a diagram follows
from the light one:

    python3 palette.py dark system-context.drawio system-context-dark.drawio
    python3 palette.py fonts system-context.svg     # adds fallbacks to Fira Sans
    python3 palette.py table                        # the colours, to pick from

Keep the tokens in step with theme.rs.
"""

import re
import sys

# Tokens of the Control Room theme: name -> (light, dark).
TOKENS = {
    "canvas": ("#f4f5f5", "#111217"),          # behind the cards
    "card": ("#ffffff", "#181b1f"),            # cards, panels, the diagram background
    "control": ("#eceef0", "#22252b"),         # buttons and fields (light: hover, apart from canvas)
    "border": ("#dcdee1", "#2a2d33"),
    "border_strong": ("#c7cacf", "#3a3e46"),
    "text": ("#24292e", "#ccccdc"),
    "text_weak": ("#5b6168", "#9a9aab"),       # secondary text, lines
    "text_faint": ("#878d95", "#6e6e80"),
    "accent": ("#3871dc", "#3d71d9"),
    # text on the accent, white in both themes (the light value differs from the
    # card's, so the two map apart)
    "on_accent": ("#fffffe", "#ffffff"),
    "accent_text": ("#1f62e0", "#6e9fff"),
    "cursor": ("#3a3f46", "#ececf4"),
    "ok": ("#1f9d55", "#3fbf72"),              # log level info
    "warn": ("#c27c0e", "#e3a93a"),
    "error": ("#d93a3a", "#f2615b"),
    "debug": ("#7b61d9", "#a68bfa"),
    "trace": ("#8e9199", "#80838b"),
}

# The trace colours, the IBM colour-blind-safe palette, the same in both themes but the
# grey. Each part of the system has one: blip blue, outpost purple, target orange,
# recordings yellow, tests magenta.
TRACES = {
    "blue": ("#648fff", "#648fff"),
    "orange": ("#fe6100", "#fe6100"),
    "magenta": ("#dc267f", "#dc267f"),
    "yellow": ("#ffb000", "#ffb000"),
    "purple": ("#785ef0", "#785ef0"),
    "grey": ("#6b7280", "#9aa0a6"),
}

# How much of a colour tints the card (theme.rs `tint`), for the fills of boxes.
TINT = (0.3, 0.42)


def _rgb(color):
    return tuple(int(color[i:i + 2], 16) for i in (1, 3, 5))


def _hex(rgb):
    return "#" + "".join(f"{round(c):02x}" for c in rgb)


def tint(card, color, amount):
    """`color` tinting `card`, as theme::tint does (a lerp of the gamma-encoded
    values)."""
    a, b = _rgb(card), _rgb(color)
    return _hex(x + (y - x) * amount for x, y in zip(a, b))


def colors():
    """All colours of the diagrams: name -> (light, dark)."""
    table = dict(TOKENS)
    table.update(TRACES)
    card_light, card_dark = TOKENS["card"]
    for name, (light, dark) in TRACES.items():
        table[f"{name}_tint"] = (tint(card_light, light, TINT[0]), tint(card_dark, dark, TINT[1]))
    return table


def dark(text):
    """`text`, a draw.io file in the light colours, in the dark ones."""
    mapping = {light.lower(): dark for light, dark in colors().values() if light != dark}

    def replace(match):
        return mapping.get(match.group(0).lower(), match.group(0))

    return re.sub(r"#[0-9a-fA-F]{6}\b", replace, text)


FONTS = "Fira Sans, Helvetica, Arial, sans-serif"
MONO = "Fira Code, Courier New, monospace"


def fonts(svg):
    """`svg` with fallbacks for the fonts of the docs, which an <img> cannot load."""
    svg = re.sub(r'font-family="Fira Sans"', f'font-family="{FONTS}"', svg)
    svg = re.sub(r'font-family="Fira Code"', f'font-family="{MONO}"', svg)
    svg = re.sub(r"font-family: Fira Sans;", f"font-family: {FONTS};", svg)
    svg = re.sub(r"font-family: Fira Code;", f"font-family: {MONO};", svg)
    return svg


def main(args):
    if args[:1] == ["dark"] and len(args) == 3:
        with open(args[1], encoding="utf-8") as f:
            text = f.read()
        with open(args[2], "w", encoding="utf-8") as f:
            f.write(dark(text))
    elif args[:1] == ["fonts"] and len(args) == 2:
        with open(args[1], encoding="utf-8") as f:
            svg = f.read()
        with open(args[1], "w", encoding="utf-8") as f:
            f.write(fonts(svg))
    elif args == ["table"]:
        for name, (light, dark_color) in colors().items():
            print(f"{name:16} {light}  {dark_color}")
    else:
        print(__doc__, file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
