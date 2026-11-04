"""Tests that the user documentation in doc/source matches the outpost: every option of
the command line is documented, the Python examples of the guide compile, and the
example the guide includes runs."""

import asyncio
import re
import subprocess
import sys
from pathlib import Path

import click
import pytest
import websockets

from blip_outpost.main import main
from blip_outpost.proto import decode_cbor_message

from blip_client import receive

OUTPOST = Path(__file__).parents[1]
DOCS = OUTPOST.parent / "doc" / "source"
CLI_PAGE = DOCS / "usage" / "reference" / "outpost-cli.rst"
GUIDE = DOCS / "usage" / "outpost.rst"

pytestmark = pytest.mark.skipif(not DOCS.is_dir(), reason="needs the repository's doc folder")


def options() -> list:
    """(command, option) for each option string of each command, e.g. ("tcp", "--listen")."""
    return [
        (name, opt)
        for name, command in main.commands.items()
        for param in command.params
        if isinstance(param, click.Option)
        for opt in param.opts + param.secondary_opts
    ]


@pytest.mark.parametrize("command, option", options())
def test_every_option_is_documented(command: str, option: str):
    page = CLI_PAGE.read_text(encoding="utf-8")
    assert f"``{option}``" in page, f"{option} of `blip-outpost {command}` is missing in {CLI_PAGE}"


@pytest.mark.parametrize("command", sorted(main.commands))
def test_every_command_is_documented(command: str):
    page = CLI_PAGE.read_text(encoding="utf-8")
    assert f"blip-outpost {command} " in page, f"`blip-outpost {command}` is missing in {CLI_PAGE}"


def python_blocks(path: Path) -> list:
    """The code of the ``.. code-block:: python`` directives of a page."""
    lines = path.read_text(encoding="utf-8").splitlines()
    blocks = []
    for index, line in enumerate(lines):
        if line.strip() != ".. code-block:: python":
            continue
        indent = len(line) - len(line.lstrip())
        body = []
        for following in lines[index + 1:]:
            if following.strip() and len(following) - len(following.lstrip()) <= indent:
                break
            body.append(following)
        text = "\n".join(body)
        blocks.append(re.sub(r"(?m)^" + " " * (indent + 3), "", text).strip("\n"))
    return blocks


def test_the_python_examples_of_the_guide_compile():
    blocks = python_blocks(GUIDE)
    assert blocks
    for number, block in enumerate(blocks, 1):
        compile(block, f"{GUIDE.name}, example {number}", "exec")


async def test_the_minimal_example_sends_values():
    # minimal.py serves on the default port; run it on one of the tests instead
    port = 3997
    script = (
        "import functools, runpy, blip_outpost\n"
        f"blip_outpost.start = functools.partial(blip_outpost.start, port={port})\n"
        "runpy.run_path('examples/minimal.py', run_name='__main__')\n"
    )
    process = subprocess.Popen([sys.executable, "-c", script], cwd=OUTPOST)
    try:
        for _ in range(50):
            try:
                ws = await websockets.connect(f"ws://127.0.0.1:{port}/ws/outpost")
                break
            except OSError:
                await asyncio.sleep(0.1)
        else:
            pytest.fail("minimal.py does not serve")
        async with ws:
            paths = set()
            for _ in range(100):
                data = decode_cbor_message(await receive(ws, 2.0))
                if data and "Value" in data:
                    paths.add(data["Value"]["path"])
                if "sensor/voltage" in paths:
                    break
            assert "sensor/voltage" in paths
    finally:
        process.terminate()
        process.wait(timeout=5)
