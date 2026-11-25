"""Sphinx configuration for the blip user documentation."""

import re
import shutil
from datetime import date
from html import escape
from pathlib import Path

import matplotlib.style
from docutils import nodes

# -- Project information -----------------------------------------------------

project = "blip"
author = "Stefan Lüthi"
copyright = f"{date.today().year}, {author}"
release = "0.1.0"
language = "en"

# Where the site is published: the web app at the root, this documentation
# under docs/ (.github/workflows/pages.yml).
site_url = "https://blip.embedded-systems.io/"

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
# relative to the old page, so links from outside keep working. The pages of the
# user guide and the software architecture moved into one section per part.
redirects = {
    "architecture/blip": "../blip/internals.html",
    "architecture/components": "../requirements/architecture.html",
    "architecture/concepts": "../overview/concepts.html",
    "architecture/index": "../overview/index.html",
    "architecture/outpost": "../outpost/internals.html",
    "architecture/overview": "../overview/index.html",
    "architecture/protocol": "../reference/protocol.html",
    "architecture/sonde": "../sonde/internals.html",
    "blip/views": "views/index.html",
    "requirements/documentation": "documentation/index.html",
    "requirements/system": "system/index.html",
    "requirements/tests": "tests/index.html",
    "usage/blip/actions": "../../blip/actions.html",
    "usage/blip/connecting": "../../blip/connecting.html",
    "usage/blip/files": "../../blip/files.html",
    "usage/blip/index": "../../blip/index.html",
    "usage/blip/layout": "../../blip/layout.html",
    "usage/blip/reference": "../../blip/controls.html",
    "usage/blip/time": "../../blip/time.html",
    "usage/blip/traces": "../../blip/traces.html",
    "usage/blip/views": "../../blip/views/index.html",
    "usage/concepts": "../overview/concepts.html",
    "usage/index": "../index.html",
    "usage/installation": "../start/installation.html",
    "usage/outpost": "../outpost/index.html",
    "usage/quickstart": "../start/quickstart.html",
    "usage/sonde": "../sonde/index.html",
}

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
    r"https://blip\.embedded-systems\.io/.*",
    r"https://github\.com/stefanluethi/blip/(edit|blob|issues).*",
]
linkcheck_anchors = False

# -- HTML output -------------------------------------------------------------

html_theme = "shibuya"
html_title = "blip documentation"
# The address of the published documentation: it gives each page its canonical
# link, and the addresses in the Open Graph tags and in the sitemap below.
html_baseurl = site_url + "docs/"
# The picture in the preview of a link to the documentation.
html_theme_options = {"og_image_url": html_baseurl + "_images/window.png"}
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
    """Copy the dark variants of the screenshots next to the light ones, which the
    figures reference, for themed-images.js to swap in. The diagrams hold both
    themes."""
    if exception or app.builder.format != "html":
        return
    images = Path(app.outdir, "_images")
    images.mkdir(exist_ok=True)
    for image in Path(app.srcdir).glob("screenshots/*-dark.png"):
        shutil.copyfile(image, images / image.name)


def _description(doctree):
    """The first paragraph of a page, cut to the length search engines show."""
    for paragraph in doctree.findall(nodes.paragraph):
        if not isinstance(paragraph.parent, nodes.section):
            continue
        text = " ".join(paragraph.astext().split())
        if len(text) > 160:
            text = text[:159].rsplit(" ", 1)[0].rstrip(",;:.") + "…"
        return text
    return None


def _add_description(app, pagename, templatename, context, doctree):
    """Describe each page to search engines and to the previews of links with
    its first paragraph, unless the page sets a description itself. shibuya
    writes it out with the Open Graph tags."""
    if doctree is None:
        return
    meta = context.get("meta") or {}
    if "description" not in meta:
        description = _description(doctree)
        if description:
            context["meta"] = {**meta, "description": description}


def _page_url(page):
    """The address of a page as in its canonical link, where Sphinx leaves out
    the ``index.html`` of a directory."""
    if page == "index" or page.endswith("/index"):
        return html_baseurl + page[: -len("index")]
    return html_baseurl + page + ".html"


def _write_sitemap(app, exception):
    """Write sitemap.xml for search engines: the web app and every page of the
    documentation. The redirects of moved pages are not source pages, so they
    stay out. A sitemap covers only what is below it, so pages.yml moves this
    one to the root of the site."""
    if exception or app.builder.name != "html":
        return
    pages = sorted(app.env.found_docs, key=lambda page: (page != "index", page))
    urls = [site_url] + [_page_url(page) for page in pages]
    lines = ['<?xml version="1.0" encoding="UTF-8"?>']
    lines.append('<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">')
    lines += [f"  <url><loc>{escape(url)}</loc></url>" for url in urls]
    lines.append("</urlset>")
    Path(app.outdir, "sitemap.xml").write_text("\n".join(lines) + "\n", encoding="utf-8")


def setup(app):
    app.add_config_value("site_url", "", "html")
    app.connect("html-page-context", _add_description)
    app.connect("build-finished", _drop_cdn_fonts)
    app.connect("build-finished", _copy_dark_images)
    app.connect("build-finished", _write_sitemap)


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
        "description": "Affected component(s): blip, blip-kit, proto (blip-proto), outpost, sonde, doc",
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
