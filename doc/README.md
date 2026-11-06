# blip documentation

The documentation of blip, the outpost and sonde — overview, guides, internals,
reference and requirements — built with [Sphinx](https://www.sphinx-doc.org) and
[sphinx-needs](https://sphinx-needs.readthedocs.io).

## Build

Requires [uv](https://docs.astral.sh/uv/) and [Graphviz](https://graphviz.org)
(`dot`, for diagrams and `needflow`).

```bash
cd doc
make html     # output: _build/html/index.html (warnings are errors)
make serve    # live-reloading preview
make linkcheck  # check the links to other sites, before each release
make clean
```

Without make: `uv run sphinx-build -W source _build/html`.

## Layout

```
source/
├── conf.py            Sphinx + sphinx-needs configuration (need types, links,
│                      redirects of moved pages)
├── index.rst          what blip is, where to start
├── start/             installation, quick start
├── overview/          how the parts fit together, the concepts they share
├── blip/              the guide of blip (a page per part of the window), its
│                      controls and internals
├── outpost/           the guide of the outpost, its examples and internals
├── sonde/             the guide of sonde and its internals
├── help/              troubleshooting, limits and safety
├── reference/         wire protocol, outpost command line and API, sonde API,
│                      file formats
├── requirements/      stakeholder (STK_*), system (REQ_*), documentation
│                      (REQ_DOC_*), architecture elements (ARCH_*), tests
│                      (TEST_*), traceability tables, authoring guide
├── changes.rst        release notes
├── diagrams/          draw.io sources (*.drawio) and their exported *.svg
└── screenshots/       screenshots of blip, rendered by `make screenshots`
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
of the simulator `blip/tests/data/demo.blip` and the preset
`blip/tests/data/demo-presets.json`, in the light (`name.png`) and the dark
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

## Keeping it right

The expectations for this documentation are in
`source/requirements/documentation.rst`. In short: update the pages in the same
commit as the code; a page that moves leaves an entry in `redirects` in
`conf.py`; the outpost's API reference comes from its docstrings, sonde's from
its headers; `outpost/tests/test_docs.py` fails when an option of
`blip-outpost` is not on `reference/outpost-cli.rst`; add each release to
`changes.rst`.
