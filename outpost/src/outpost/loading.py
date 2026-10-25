"""Loads the protocol named on the command line and converts its options."""

import functools
import importlib
import itertools
import importlib.util
import inspect
import sys
import types
import typing
from pathlib import Path
from typing import Any, Callable, Dict, Iterable, List, Optional, Tuple

from outpost.protocol import Protocol


# Numbers the module names of protocol files, which may share their names
_file_numbers = itertools.count()


class LoadError(Exception):
    """The protocol or its options could not be loaded, the message says why."""


def builtin_protocols() -> Dict[str, Callable[..., Protocol]]:
    """The protocols that come with the outpost by name."""
    from outpost.protocols.fast import FastProtocol
    from outpost.protocols.replay import ReplayProtocol
    from outpost.protocols.simulator import SimulatorProtocol
    from outpost.protocols.socket import SocketProtocol

    return {
        "simulator": SimulatorProtocol,
        "fast": FastProtocol,
        "replay": ReplayProtocol,
        "tcp": functools.partial(SocketProtocol, mode="tcp"),
        "udp": functools.partial(SocketProtocol, mode="udp"),
    }


def _protocol_classes(module: types.ModuleType) -> List[type]:
    return [
        member
        for member in vars(module).values()
        if isinstance(member, type)
        and issubclass(member, Protocol)
        and member is not Protocol
        and member.__module__ == module.__name__
    ]


def _import_file(path: Path) -> types.ModuleType:
    """Imports the protocol file at `path` under a private module name, so a file named
    like another module, e.g. ``serial.py``, neither replaces that module nor imports
    itself in its place. Modules next to the file can be imported, unless an installed
    module has the same name."""
    name = f"_blip_protocol_{next(_file_numbers)}_{path.stem}"
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise LoadError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    folder = str(path.resolve().parent)
    # after the installed modules, which keep their names
    if folder not in sys.path:
        sys.path.append(folder)
    sys.modules[name] = module
    try:
        spec.loader.exec_module(module)
    except Exception as e:
        del sys.modules[name]
        raise LoadError(f"error loading {path}: {type(e).__name__}: {e}") from e
    return module


def load_protocol(spec: str) -> Callable[..., Protocol]:
    """The protocol named by `spec`: a built-in name, or a Python file or module
    followed by ``:ClassName``. Without a class name the module must define exactly one
    protocol."""
    builtins = builtin_protocols()
    if spec in builtins:
        return builtins[spec]

    location, class_name = parse_spec(spec)
    path = Path(location)
    if location.endswith(".py") or path.is_file():
        if not path.is_file():
            raise LoadError(f"no file {location}")
        module = _import_file(path)
    else:
        try:
            module = importlib.import_module(location)
        except Exception as e:
            known = ", ".join(builtins)
            raise LoadError(
                f"unknown protocol '{spec}': not one of {known}, a Python file or an "
                f"importable module ({type(e).__name__}: {e})"
            ) from e

    if class_name is not None:
        protocol = getattr(module, class_name, None)
        if not (isinstance(protocol, type) and issubclass(protocol, Protocol)):
            raise LoadError(f"{location} defines no protocol class '{class_name}'")
        return protocol
    classes = _protocol_classes(module)
    if len(classes) != 1:
        names = ", ".join(cls.__name__ for cls in classes) or "none"
        raise LoadError(
            f"{location} defines {len(classes)} protocols ({names}), name one with "
            f"{location}:ClassName"
        )
    return classes[0]


def _options(factory: Callable[..., Protocol]) -> List[inspect.Parameter]:
    """The keyword arguments of the constructor of a protocol, without the ones a
    partial already sets."""
    signature = inspect.signature(factory)
    fixed = factory.keywords if isinstance(factory, functools.partial) else {}
    return [
        parameter
        for name, parameter in signature.parameters.items()
        if name not in fixed
        and parameter.kind not in (parameter.VAR_POSITIONAL, parameter.VAR_KEYWORD)
    ]


def _hints(factory: Callable[..., Protocol]) -> Dict[str, Any]:
    target = factory.func if isinstance(factory, functools.partial) else factory
    init = target.__init__ if isinstance(target, type) else target
    try:
        return typing.get_type_hints(init)
    except Exception:
        return {}


def _unwrap_optional(hint: Any) -> Any:
    if typing.get_origin(hint) in (typing.Union, getattr(types, "UnionType", None)):
        args = [arg for arg in typing.get_args(hint) if arg is not type(None)]
        if len(args) == 1:
            return args[0]
    return hint


def _convert(name: str, text: str, hint: Any) -> Any:
    hint = _unwrap_optional(hint)
    try:
        if hint is bool:
            lowered = text.lower()
            if lowered in ("1", "true", "yes", "on"):
                return True
            if lowered in ("0", "false", "no", "off"):
                return False
            raise ValueError("expected true or false")
        if hint is int:
            return int(text, 0)
        if hint is float:
            return float(text)
        if hint is Path:
            return Path(text)
    except ValueError as e:
        raise LoadError(f"option {name}: invalid value '{text}' ({e})") from e
    return text


def describe_options(factory: Callable[..., Protocol]) -> str:
    """The options of a protocol as shown by ``--protocol-help``."""
    hints = _hints(factory)
    lines = []
    for parameter in _options(factory):
        hint = _unwrap_optional(hints.get(parameter.name, str))
        type_name = getattr(hint, "__name__", str(hint))
        if parameter.default is parameter.empty:
            lines.append(f"  {parameter.name}: {type_name} (required)")
        else:
            lines.append(f"  {parameter.name}: {type_name} = {parameter.default!r}")
    return "\n".join(lines) or "  (no options)"


def create_protocol(factory: Callable[..., Protocol], options: Iterable[str]) -> Protocol:
    """Creates the protocol with the ``key=value`` options, converted by the type hints
    of its constructor."""
    known = {parameter.name: parameter for parameter in _options(factory)}
    hints = _hints(factory)
    kwargs: Dict[str, Any] = {}
    for option in options:
        name, equals, text = option.partition("=")
        name = name.strip()
        if not equals:
            raise LoadError(f"option '{option}' is not of the form key=value")
        if name not in known:
            available = ", ".join(known) or "none"
            raise LoadError(f"unknown option '{name}', the protocol has: {available}")
        kwargs[name] = _convert(name, text, hints.get(name, str))
    missing = [
        name for name, parameter in known.items()
        if parameter.default is parameter.empty and name not in kwargs
    ]
    if missing:
        raise LoadError(f"missing option(s): {', '.join(missing)}")
    try:
        return factory(**kwargs)
    except LoadError:
        raise
    except Exception as e:
        raise LoadError(f"cannot create the protocol: {type(e).__name__}: {e}") from e


def parse_spec(spec: str) -> Tuple[str, Optional[str]]:
    """Splits a ``location:ClassName`` spec into the location and the class name, if
    any. The colon of a Windows drive does not count."""
    head, colon, tail = spec.rpartition(":")
    if colon and head and len(head) > 1 and not ("/" in tail or "\\" in tail):
        return head, tail
    return spec, None
