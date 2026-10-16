"""Tests for loading protocols named on the command line and their options."""

from pathlib import Path

import pytest

from outpost.loading import (
    LoadError,
    create_protocol,
    describe_options,
    load_protocol,
    parse_spec,
)
from outpost.protocols.simulator import SimulatorProtocol
from outpost.protocols.socket import SocketProtocol

USER_PROTOCOL = '''
from pathlib import Path
from typing import Optional

from outpost import Protocol


class Target(Protocol):
    def __init__(self, host: str, port: int = 80, fast: bool = False,
                 rate: float = 1.0, log: Optional[Path] = None) -> None:
        self.host, self.port, self.fast, self.rate, self.log = host, port, fast, rate, log

    async def run(self) -> None:
        pass
'''


def write_protocol(tmp_path: Path, source: str = USER_PROTOCOL, name: str = "target.py") -> Path:
    path = tmp_path / name
    path.write_text(source, encoding="utf-8")
    return path


def test_builtin_protocols_are_found_by_name():
    assert load_protocol("simulator") is SimulatorProtocol
    tcp = create_protocol(load_protocol("tcp"), [])
    assert isinstance(tcp, SocketProtocol) and tcp.mode == "tcp"


def test_specs_split_off_the_class_name_but_not_a_drive():
    assert parse_spec("target.py") == ("target.py", None)
    assert parse_spec("target.py:Target") == ("target.py", "Target")
    assert parse_spec("my_pkg.target:Target") == ("my_pkg.target", "Target")
    assert parse_spec(r"C:\work\target.py") == (r"C:\work\target.py", None)
    assert parse_spec(r"C:\work\target.py:Target") == (r"C:\work\target.py", "Target")


def test_a_file_with_one_protocol_needs_no_class_name(tmp_path):
    path = write_protocol(tmp_path)
    assert load_protocol(str(path)).__name__ == "Target"
    assert load_protocol(f"{path}:Target").__name__ == "Target"


def test_a_file_with_several_protocols_needs_a_class_name(tmp_path):
    source = USER_PROTOCOL + "\n\nclass Other(Target):\n    pass\n"
    path = write_protocol(tmp_path, source, "several.py")
    with pytest.raises(LoadError, match="defines 2 protocols"):
        load_protocol(str(path))
    assert load_protocol(f"{path}:Other").__name__ == "Other"
    with pytest.raises(LoadError, match="no protocol class 'Missing'"):
        load_protocol(f"{path}:Missing")


def test_a_file_named_like_a_module_does_not_shadow_it(tmp_path):
    import json
    import sys

    (tmp_path / "target_helpers.py").write_text("PORT = 4242\n", encoding="utf-8")
    source = (
        "import json\nfrom target_helpers import PORT\nfrom outpost import Protocol\n\n"
        "class Json(Protocol):\n"
        "    def __init__(self, port: int = PORT) -> None:\n"
        "        self.text = json.dumps({'port': port})\n\n"
        "    async def run(self) -> None:\n"
        "        pass\n"
    )
    path = write_protocol(tmp_path, source, "json.py")
    protocol = create_protocol(load_protocol(str(path)), [])
    # the protocol imported the json of the standard library, which stays registered
    assert protocol.text == '{"port": 4242}'
    assert sys.modules["json"] is json


def test_unknown_protocols_are_reported():
    with pytest.raises(LoadError, match="unknown protocol"):
        load_protocol("no_such_module_here")
    with pytest.raises(LoadError, match="no file"):
        load_protocol("missing.py")


def test_options_are_converted_by_their_type_hints(tmp_path):
    factory = load_protocol(str(write_protocol(tmp_path)))
    target = create_protocol(
        factory, ["host=192.168.1.70", "port=0x50", "fast=yes", "rate=2.5", "log=out.txt"]
    )
    assert target.host == "192.168.1.70"
    assert target.port == 80
    assert target.fast is True
    assert target.rate == 2.5
    assert target.log == Path("out.txt")


def test_wrong_options_are_reported(tmp_path):
    factory = load_protocol(str(write_protocol(tmp_path)))
    with pytest.raises(LoadError, match="missing option"):
        create_protocol(factory, [])
    with pytest.raises(LoadError, match="unknown option 'colour'"):
        create_protocol(factory, ["host=a", "colour=red"])
    with pytest.raises(LoadError, match="option port: invalid value"):
        create_protocol(factory, ["host=a", "port=eighty"])
    with pytest.raises(LoadError, match="not of the form"):
        create_protocol(factory, ["host"])


def test_options_are_described(tmp_path):
    factory = load_protocol(str(write_protocol(tmp_path)))
    description = describe_options(factory)
    assert "host: str (required)" in description
    assert "port: int = 80" in description
    assert "log: Path = None" in description
    # the mode of tcp is set already
    assert "mode" not in describe_options(load_protocol("tcp"))
