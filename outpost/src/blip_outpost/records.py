"""Dataclasses as records: the units of their fields, declared with `Unit`::

    @dataclass
    class Imu:
        acc: Annotated[List[float], Unit("m/s²")]
        valid: bool

    outpost.value("imu", Imu([0.0, 0.0, 9.81], True))   # traces imu.acc and imu.valid

The fields of a dataclass become the fields of the record, as the keys of a dict do; a
field that is a dataclass itself becomes a nested record, e.g. ``imu.pose.x``.
"""

import dataclasses
import functools
import logging
import sys
import types
import typing
from typing import Any, List, Tuple

from blip_outpost.actions import Unit

logger = logging.getLogger(__name__)


@functools.lru_cache(maxsize=None)
def field_units(cls: type) -> Tuple[Tuple[str, str], ...]:
    """The fields of the dataclass `cls` that declare a `Unit`, also of the dataclasses
    in its fields, as (path after the record, unit), e.g. ``("pose.x", "m")``."""
    units: List[Tuple[str, str]] = []
    for f, hint in zip(dataclasses.fields(cls), _hints(cls)):
        unit, inner = _unit_of(hint)
        if unit:
            units.append((f.name, unit))
        if isinstance(inner, type) and dataclasses.is_dataclass(inner):
            units.extend((f"{f.name}.{path}", u) for path, u in field_units(inner))
    return tuple(units)


def _hints(cls: type) -> List[Any]:
    """The annotations of the fields of `cls`, resolved where they are strings, e.g.
    with ``from __future__ import annotations``. A field whose annotation does not
    resolve, e.g. as it names a class local to a function, is left out with a warning,
    the others keep their units."""
    fields = dataclasses.fields(cls)
    try:
        hints = typing.get_type_hints(cls, include_extras=True)
        return [hints.get(f.name, f.type) for f in fields]
    except Exception:
        pass
    module = sys.modules.get(cls.__module__)
    globals_ = dict(vars(module)) if module is not None else {}
    resolved: List[Any] = []
    unresolved: List[str] = []
    for f in fields:
        if not isinstance(f.type, str):
            resolved.append(f.type)
            continue
        try:
            # as get_type_hints does it, field by field
            resolved.append(eval(f.type, globals_, dict(vars(cls))))
        except Exception:
            resolved.append(None)
            unresolved.append(f.name)
    if unresolved:
        logger.warning(f"The units of {', '.join(unresolved)} of {cls.__qualname__} are not sent: "
                       "their annotations do not resolve")
    return resolved


def _unit_of(hint: Any) -> Tuple[str, Any]:
    """The unit an annotation declares, and the type it annotates, without `Optional`."""
    unit = ""
    if typing.get_origin(hint) is typing.Annotated:
        for extra in hint.__metadata__:
            if isinstance(extra, Unit):
                unit = extra.unit
        hint = typing.get_args(hint)[0]
    # Optional[X] and X | None annotate X
    if typing.get_origin(hint) in (typing.Union, types.UnionType):
        args = [a for a in typing.get_args(hint) if a is not type(None)]
        if len(args) == 1:
            inner_unit, inner = _unit_of(args[0])
            return unit or inner_unit, inner
    return unit, hint
