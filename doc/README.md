# blip documentation

User documentation for blip — software architecture, usage guide and
requirements — built with [Sphinx](https://www.sphinx-doc.org) and
[sphinx-needs](https://sphinx-needs.readthedocs.io).

## Build

Requires [uv](https://docs.astral.sh/uv/) and [Graphviz](https://graphviz.org)
(`dot`, for diagrams and `needflow`).

```bash
cd doc
make html     # output: _build/html/index.html (warnings are errors)
make serve    # live-reloading preview
make clean
```

Without make: `uv run sphinx-build -W source _build/html`.

## Layout

```
source/
├── conf.py            Sphinx + sphinx-needs configuration (need types, links)
├── index.rst
├── usage/             installation, quick start, concepts, the guides of
│   └── blip/          blip (one page per part of the window), outpost, sonde
├── architecture/      overview, concepts, wire protocol, blip, outpost, sonde,
│                      components (ARCH_*)
├── diagrams/          draw.io sources (*.drawio) and their exported *.svg
├── screenshots/       screenshots of blip, rendered by `make screenshots`
└── requirements/      stakeholder (STK_*), system (REQ_*), tests (TEST_*),
                       traceability tables, authoring guide
```

## Diagrams

The diagrams are drawn with [draw.io](https://www.drawio.com) in the colours of
blip (`blip/src/app/theme.rs`). Edit the `.drawio` file (in the desktop app, or
the draw.io VS Code extension), then export the SVGs next to the sources:

```bash
make diagrams   # needs the draw.io desktop app (`drawio`) on the PATH
```

Each diagram is exported twice: `name.svg` in the light colours and
`name-dark.svg`, whose colours `source/diagrams/palette.py` maps to those of the
dark theme. `_static/themed-images.js` shows the variant of the theme of the page.
The SVGs are committed, so building the documentation does not need draw.io.

Draw with the light colours of the palette only, so that each maps to its dark
twin; `python3 source/diagrams/palette.py table` lists them. Each part has one
colour of the trace palette, as a tinted fill with the colour as border: blip
blue, outpost purple, target orange, recordings yellow, tests magenta, others
grey. Text is `text`, lines `text_weak`, plain boxes the `card`. Keep the
palette in step with `theme.rs`.

Include a diagram with a figure of class `diagram`, linked to the full-size
image:

```rst
.. figure:: /diagrams/system-context.svg
   :target: ../_images/system-context.svg
   :class: diagram
   :alt: What the diagram shows, in words.

   Caption.
```

## Screenshots

The screenshots in `source/screenshots` are rendered by tests of blip with
[egui_kittest](https://docs.rs/egui_kittest) and wgpu, from the 30 s recording
of the simulator `blip/tests/data/demo.blip` and the layout
`blip/tests/data/demo-layout.json`, in the light (`name.png`) and the dark
theme (`name-dark.png`):

```bash
make screenshots   # cargo test --release --features screenshots, then optipng
```

They are committed; render them again after a change to the UI. The tests are
in `blip/src/app/screenshots.rs`: the window is rendered connected to a
WebSocket server in the test that sends the recording, on port 3000 if it is
free (else the connections panel shows another port), the views and panels on
their own. Times show in UTC. wgpu needs a GPU driver or a software renderer,
e.g. `vulkan-swrast` (Arch) or `mesa-vulkan-drivers` (Debian, Ubuntu).

Include a screenshot with a figure of class `screenshot`, linked to the
full-size image; `_static/themed-images.js` shows the dark one in dark mode:

```rst
.. figure:: /screenshots/window.png
   :target: ../_images/window.png
   :class: screenshot
   :alt: What the screenshot shows, in words.
```

To record a new demo, run the simulator with `--record` while a script invokes
its actions, and replace `demo.blip`; keep it short, as it is committed.
