# Fonts

The fonts of the blip UI, embedded into the app.

| File                     | Font        | Licence                                   |
| ------------------------ | ----------- | ----------------------------------------- |
| `Inter-Regular.ttf`      | Inter 4.001 | SIL Open Font License, `Inter-OFL.txt`      |
| `RobotoMono-Regular.ttf` | Roboto Mono | SIL Open Font License, `RobotoMono-OFL.txt` |

Both are made from the variable fonts of the Google Fonts repository
(`ofl/inter/Inter[opsz,wght].ttf` and `ofl/robotomono/RobotoMono[wght].ttf`),
fixed at the regular weight and subset to Latin, Greek and common symbols to keep
the web build small. Glyphs outside the subset fall back to egui's default fonts.

```bash
U="U+0020-007E,U+00A0-024F,U+0370-03FF,U+2000-206F,U+2070-209F,U+20A0-20CF,U+2100-214F,U+2190-21FF,U+2200-22FF,U+2300-23FF,U+25A0-25FF"
uvx --from fonttools fonttools varLib.instancer "Inter[opsz,wght].ttf" wght=400 opsz=14 -o inter.ttf
uvx --from fonttools fonttools varLib.instancer "RobotoMono[wght].ttf" wght=400 -o mono.ttf
uvx --from fonttools pyftsubset inter.ttf --unicodes="$U" \
    --layout-features='kern,liga,calt,tnum,case,ccmp,locl,mark,mkmk' --output-file=Inter-Regular.ttf
uvx --from fonttools pyftsubset mono.ttf --unicodes="$U" \
    --layout-features='kern,ccmp,locl,mark,mkmk' --output-file=RobotoMono-Regular.ttf
```
