"""The colours of blip (kit/src/widgets/theme.rs) for the draw.io diagrams.

Each diagram is an editable SVG (name.drawio.svg) that draw.io opens and saves as it
is. Its colours are those of this palette, each written `light-dark(light,dark)`, so
that the SVG shows the theme of the page (theme.css sets the color-scheme of its
<img>). The trace colours, the same in both themes but the grey, stay plain.

    python3 palette.py table                 # the colours, to paste into draw.io
    python3 palette.py check *.drawio.svg    # colours outside the palette or light only

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


def _pairs():
    """The colours of the diagrams as draw.io writes them: plain for those the same
    in both themes, else `light-dark(light,dark)`."""
    return {
        light.lower() if light == dark else f"light-dark({light},{dark})".lower()
        for light, dark in colors().values()
    }


def check(svg):
    """What is wrong with `svg`, a .drawio.svg: the colours of its image that are
    not in the palette, or that would not change with the theme."""
    problems = []
    if not re.search(r"<svg[^>]*color-scheme:\s*light dark", svg):
        problems.append("the <svg> has no `color-scheme: light dark`")
    # the image, without the diagram that draw.io embeds in it
    image = re.sub(r'\bcontent="[^"]*"', "", svg)
    image = re.sub(r"rgb\((\d+), ?(\d+), ?(\d+)\)", lambda m: _hex(int(c) for c in m.groups()), image)
    image = re.sub(r"light-dark\(\s*(#\w+)\s*,\s*(#\w+)\s*\)", r"light-dark(\1,\2)", image)
    allowed = _pairs()
    found = re.findall(r"light-dark\(#\w+,#\w+\)|#[0-9a-fA-F]{6}\b", image)
    dark_of = {light.lower(): dark for light, dark in colors().values()}
    for colour in sorted({c.lower() for c in found} - allowed):
        if colour in dark_of:
            problems.append(f"colour {colour} is light only: light-dark({colour},{dark_of[colour]})")
        else:
            problems.append(f"colour {colour} is not in the palette")
    return problems


def main(args):
    if args == ["table"]:
        for name, (light, dark_color) in colors().items():
            colour = light if light == dark_color else f"light-dark({light},{dark_color})"
            print(f"{name:16} {colour}")
    elif args[:1] == ["check"] and len(args) > 1:
        failed = False
        for path in args[1:]:
            with open(path, encoding="utf-8") as f:
                for problem in check(f.read()):
                    print(f"{path}: {problem}", file=sys.stderr)
                    failed = True
        return 1 if failed else 0
    else:
        print(__doc__, file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
