"""Sphinx configuration for the blip user documentation."""

import re
from datetime import date
from pathlib import Path

import matplotlib.style

# -- Project information -----------------------------------------------------

project = "blip"
author = "Stefan Lüthi"
copyright = f"{date.today().year}, {author}"
release = "0.1.0"

# -- General configuration ---------------------------------------------------

extensions = [
    "sphinx.ext.graphviz",
    "sphinx.ext.todo",
    "sphinx_needs",
]

templates_path = ["_templates"]
exclude_patterns = ["_build"]

todo_include_todos = True
graphviz_output_format = "svg"

# -- HTML output -------------------------------------------------------------

html_theme = "shibuya"
html_title = "blip documentation"
html_static_path = ["_static"]
# Fira Sans for text, Fira Code for code; fonts.css self-hosts them (no Google
# Fonts, for GDPR) and maps them onto shibuya's font variables.
# _templates/partials/webfonts.html drops shibuya's own Google Fonts link.
# theme.css puts shibuya's font sizes and spacing on one scale; needs.css
# adapts sphinx-needs to shibuya's colors and to that scale.
html_css_files = ["fonts.css", "theme.css", "needs.css"]


def _drop_cdn_fonts(app, exception):
    """Remove shibuya's flag emoji @font-face, which loads from jsdelivr."""
    if exception or app.builder.format != "html":
        return
    css = Path(app.outdir, "_static", "shibuya.css")
    if css.exists():
        text = css.read_text(encoding="utf-8")
        css.write_text(
            re.sub(r"@font-face\{[^}]*cdn\.jsdelivr\.net[^}]*\}", "", text),
            encoding="utf-8",
        )


def setup(app):
    app.connect("build-finished", _drop_cdn_fonts)


# -- sphinx-needs ------------------------------------------------------------
# Requirement hierarchy:
#   stakeholder (STK) -> system requirement (REQ) -> architecture element (ARCH)
#   test case (TEST) verifies requirements.

needs_id_required = True
needs_id_regex = r"^(STK|REQ|ARCH|TEST)_[A-Z0-9_]+$"

needs_types = [
    {
        "directive": "stk",
        "title": "Stakeholder Requirement",
        "prefix": "STK_",
        "color": "#BFD8D2",
        "style": "node",
    },
    {
        "directive": "req",
        "title": "Requirement",
        "prefix": "REQ_",
        "color": "#FEDCD2",
        "style": "node",
    },
    {
        "directive": "arch",
        "title": "Architecture Element",
        "prefix": "ARCH_",
        "color": "#DCB239",
        "style": "component",
    },
    {
        "directive": "test",
        "title": "Test Case",
        "prefix": "TEST_",
        "color": "#DF744A",
        "style": "folder",
    },
]

needs_fields = {
    "status": {
        "description": "Lifecycle state of the need",
        "schema": {
            "type": "string",
            "enum": ["draft", "approved", "implemented", "verified", "rejected"],
        },
    },
    "component": {
        "description": "Affected component(s): blip, proto, outpost",
        "schema": {"type": "string"},
    },
    "priority": {
        "description": "Priority of the need",
        "schema": {"type": "string", "enum": ["high", "medium", "low"]},
    },
    "rationale": {
        "description": "Justification for the need",
        "schema": {"type": "string"},
    },
}

needs_links = {
    "satisfies": {
        "description": "req -> stk, arch -> req",
        "incoming": "satisfied by",
        "outgoing": "satisfies",
    },
    "implements": {
        "description": "Code-level item -> arch",
        "incoming": "implemented by",
        "outgoing": "implements",
    },
    "verifies": {
        "description": "test -> req",
        "incoming": "verified by",
        "outgoing": "verifies",
        "color": "#DF744A",
        "style": "dashed",
    },
}

# Render needflow with graphviz (already required by sphinx.ext.graphviz);
# the default plantuml engine is not installed.
needs_flow_engine = "graphviz"

# Transparent needflow and needpie backgrounds, so needs.css can adapt them to
# dark mode. needpie resets the matplotlib style per chart, so it needs
# `:style: transparent` rather than a global rcParams setting.
needs_graphviz_styles = {"default": {"graph": {"bgcolor": "transparent"}}}
matplotlib.style.library["transparent"] = {"savefig.transparent": True}

# Write needs.json for traceability tooling / CI checks.
needs_build_json = True
