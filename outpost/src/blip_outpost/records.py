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
import types
import typing
from typing import Any, List, Tuple

from blip_outpost.actions import Unit


@functools.lru_cache(maxsize=None)
def field_units(cls: type) -> Tuple[Tuple[str, str], ...]:
    """The fields of the dataclass `cls` that declare a `Unit`, also of the dataclasses
    in its fields, as (path after the record, unit), e.g. ``("pose.x", "m")``."""
    units: List[Tuple[str, str]] = []
    try:
        hints = typing.get_type_hints(cls, include_extras=True)
    except Exception:
        # e.g. a forward reference that does not resolve: no units then
        hints = {}
    for f in dataclasses.fields(cls):
        unit, inner = _unit_of(hints.get(f.name, f.type))
        if unit:
            units.append((f.name, unit))
        if isinstance(inner, type) and dataclasses.is_dataclass(inner):
            units.extend((f"{f.name}.{path}", u) for path, u in field_units(inner))
    return tuple(units)


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
