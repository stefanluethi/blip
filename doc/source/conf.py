"""Sphinx configuration for the blip user documentation."""

import re
import shutil
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
    "sphinx.ext.autodoc",
    "sphinx.ext.graphviz",
    "sphinx.ext.todo",
    "sphinx_needs",
    "sphinx_reredirects",
]

templates_path = ["_templates"]
exclude_patterns = ["_build"]

todo_include_todos = True
graphviz_output_format = "svg"

# The docstrings of blip_outpost name code in single backticks.
default_role = "py:obj"
autodoc_member_order = "bysource"
autodoc_typehints = "signature"

# A page that moves or is deleted leaves a redirect here, old name -> new URL,
# relative to the old page, so links from outside keep working, e.g.
#   "usage/old-name": "new-name.html",
redirects = {}

# Links to other sites are checked by `make linkcheck`. Left out: the images the
# figures link to (they exist only in the HTML build), addresses on the reader's
# machine, and the hosted app and the GitHub links, which answer only once
# published.
linkcheck_ignore = [
    r"(\.\./)*_images/.*",
    r"https?://(127\.0\.0\.1|localhost)[:/].*",
    r"wss?://.*",
    # answers link checkers with 403
    r"https://trunkrs\.dev/?",
    r"https://stefanluethi\.github\.io/blip/.*",
    r"https://github\.com/stefanluethi/blip/(edit|blob|issues).*",
]
linkcheck_anchors = False

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
# themed-images.js shows the dark variant of the diagrams and screenshots in dark mode.
html_js_files = ["themed-images.js"]
# "Edit this page" and "Report a problem" on each page. The repo-stats sidebar of
# shibuya is left out, as it calls the GitHub API from the reader's browser.
html_context = {
    "source_type": "github",
    "source_user": "stefanluethi",
    "source_repo": "blip",
    "source_version": "main",
    "source_docs_path": "/doc/source/",
}
html_sidebars = {
    "**": [
        "sidebars/localtoc.html",
        "sidebars/edit-this-page.html",
        "sidebars/report-problem.html",
    ]
}


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


def _copy_dark_images(app, exception):
    """Copy the dark variants of the diagrams and screenshots next to the light
    ones, which the figures reference, for themed-images.js to swap in."""
    if exception or app.builder.format != "html":
        return
    images = Path(app.outdir, "_images")
    images.mkdir(exist_ok=True)
    for pattern in ("diagrams/*-dark.svg", "screenshots/*-dark.png"):
        for image in Path(app.srcdir).glob(pattern):
            shutil.copyfile(image, images / image.name)


def setup(app):
    app.connect("build-finished", _drop_cdn_fonts)
    app.connect("build-finished", _copy_dark_images)


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
        "description": "Affected component(s): blip, proto, outpost, sonde, doc",
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
