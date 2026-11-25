#!/usr/bin/env python3
"""Formats a Rust file after Claude edited it, so that `cargo fmt --check` in check.sh
does not fail and cost a turn. rustfmt also formats the modules the file declares.

Runs as a PostToolUse hook on Edit and Write. It reads the hook input as JSON on stdin
and never blocks: a file that does not parse yet stays as it is.
"""

import json
import subprocess
import sys

path = json.load(sys.stdin).get("tool_input", {}).get("file_path", "")
if path.endswith(".rs"):
    subprocess.run(["rustfmt", "--edition", "2024", path], capture_output=True)
