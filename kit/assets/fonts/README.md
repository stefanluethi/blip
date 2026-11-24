# Fonts

The fonts of the blip UI, embedded into the app. They are the faces of the blip
documentation as well.

| File                   | Font      | Licence                                   |
| ---------------------- | --------- | ----------------------------------------- |
| `FiraSans-Book.ttf`    | Fira Sans | SIL Open Font License, `FiraSans-OFL.txt` |
| `FiraCode-Regular.ttf` | Fira Code | SIL Open Font License, `FiraCode-OFL.txt` |

Fira Sans Book (weight 350, between Light and Regular) is made from `FiraSans-Book.ttf` of
the Fira Sans 4.301 release by bBox Type (<https://github.com/bBoxType/FiraSans>; Google
Fonts has no Book weight). Its tabular figures are made the default with
[opentype-feature-freezer](https://github.com/twardoch/fonttools-opentype-feature-freezer),
since egui cannot turn on OpenType features, so that changing values keep their place.
Fira Code is made from the variable `ofl/firacode/FiraCode[wght].ttf` of the Google Fonts
repository, fixed at the regular weight. Both are subset to Latin, Greek and common symbols
to keep the web build small. Glyphs outside the subset fall back to egui's default fonts.

```bash
U="U+0020-007E,U+00A0-024F,U+0370-03FF,U+2000-206F,U+2070-209F,U+20A0-20CF,U+2100-214F,U+2190-21FF,U+2200-22FF,U+2300-23FF,U+25A0-25FF"
uvx --from opentype-feature-freezer pyftfeatfreeze -f tnum FiraSans-Book.ttf frozen.ttf
uvx --from fonttools pyftsubset frozen.ttf --unicodes="$U" \
    --layout-features='kern,liga,case,ccmp,locl,mark,mkmk' --output-file=FiraSans-Book.ttf
uvx --from fonttools fonttools varLib.instancer "FiraCode[wght].ttf" wght=400 -o code.ttf
uvx --from fonttools pyftsubset code.ttf --unicodes="$U" \
    --layout-features='kern,ccmp,locl,mark,mkmk' --output-file=FiraCode-Regular.ttf
```
