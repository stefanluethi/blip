# Fonts

The fonts of the blip UI, embedded into the app. They are the faces of the blip
documentation as well.

| File                   | Font      | Licence                                   |
| ---------------------- | --------- | ----------------------------------------- |
| `FiraSans-Regular.ttf` | Fira Sans | SIL Open Font License, `FiraSans-OFL.txt` |
| `FiraCode-Regular.ttf` | Fira Code | SIL Open Font License, `FiraCode-OFL.txt` |

Both are made from the fonts of the Google Fonts repository
(`ofl/firasans/FiraSans-Regular.ttf` and the variable `ofl/firacode/FiraCode[wght].ttf`,
fixed at the regular weight), subset to Latin, Greek and common symbols to keep the
web build small. Glyphs outside the subset fall back to egui's default fonts.

```bash
U="U+0020-007E,U+00A0-024F,U+0370-03FF,U+2000-206F,U+2070-209F,U+20A0-20CF,U+2100-214F,U+2190-21FF,U+2200-22FF,U+2300-23FF,U+25A0-25FF"
uvx --from fonttools pyftsubset FiraSans-Regular.ttf --unicodes="$U" \
    --layout-features='kern,liga,tnum,case,ccmp,locl,mark,mkmk' --output-file=FiraSans-Regular.ttf
uvx --from fonttools fonttools varLib.instancer "FiraCode[wght].ttf" wght=400 -o code.ttf
uvx --from fonttools pyftsubset code.ttf --unicodes="$U" \
    --layout-features='kern,ccmp,locl,mark,mkmk' --output-file=FiraCode-Regular.ttf
```
