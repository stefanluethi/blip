"""Sphinx configuration for the blip user documentation."""

from datetime import date

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

html_theme = "furo"
html_title = "blip documentation"
html_static_path = ["_static"]

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
        "description": "Affected component(s): blip, proto, bridge",
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

# Write needs.json for traceability tooling / CI checks.
needs_build_json = True
