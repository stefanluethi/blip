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
├── architecture/      overview, components (ARCH_*), wire protocol
├── usage/             installation, running, UI
└── requirements/      stakeholder (STK_*), system (REQ_*), tests (TEST_*),
                       traceability tables, authoring guide
```

See `source/requirements/authoring.rst` for how to write requirements.
