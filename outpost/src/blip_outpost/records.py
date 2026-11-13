"""Dataclasses as records: what their fields declare for blip, with `Annotated`::

    @dataclass
    class Drive:
        speed: Annotated[int, Unit("rpm"), Range(0, 3000), Description("Shaft speed")]
        mode: Annotated[int, Labels({0: "idle", 1: "running"})]
        valid: bool

    outpost.value("drive", Drive(1200, 1, True))   # traces drive.speed, drive.mode, drive.valid

`Unit` gives the unit, `Description` a text, `Labels` the names of integer values and
`Range` the expected range (when both `min` and `max` are set; `step` is ignored). The
first annotation of each kind wins.

The fields of a dataclass become the fields of the record, as the keys of a dict do; a
field that is a dataclass itself becomes a nested record, e.g. ``drive.pose.x``.
"""

import dataclasses
import functools
import logging
import sys
import types
import typing
from typing import Any, List, Mapping, Optional, Tuple

from blip_outpost.actions import Range, Unit

logger = logging.getLogger(__name__)


@dataclasses.dataclass(frozen=True, eq=False)
class Description:
    """The description of a trace, e.g. a field of a record."""
    text: str


@dataclasses.dataclass(frozen=True, eq=False)
class Labels:
    """The names of the integer values of a trace, e.g. ``Labels({0: "idle", 1: "running"})``."""
    labels: Mapping[int, str]


@dataclasses.dataclass(frozen=True)
class FieldMetadata:
    """What the annotations of a field declare."""
    unit: str = ""
    description: str = ""
    labels: Optional[Mapping[int, str]] = None
    value_range: Optional[Tuple[float, float]] = None

    def __bool__(self) -> bool:
        return bool(self.unit or self.description or self.labels or self.value_range is not None)


@functools.lru_cache(maxsize=None)
def field_metadata(cls: type) -> Tuple[Tuple[str, FieldMetadata], ...]:
    """The fields of the dataclass `cls` that declare metadata, also of the dataclasses
    in its fields, as (path after the record, metadata), e.g. ``("pose.x", FieldMetadata("m"))``."""
    found: List[Tuple[str, FieldMetadata]] = []
    for f, hint in zip(dataclasses.fields(cls), _hints(cls)):
        meta, inner = _metadata_of(hint)
        if meta:
            found.append((f.name, meta))
        if isinstance(inner, type) and dataclasses.is_dataclass(inner):
            found.extend((f"{f.name}.{path}", m) for path, m in field_metadata(inner))
    return tuple(found)


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


def _metadata_of(hint: Any) -> Tuple[FieldMetadata, Any]:
    """The metadata an annotation declares, and the type it annotates, without `Optional`."""
    unit = ""
    description = ""
    labels: Optional[Mapping[int, str]] = None
    value_range: Optional[Tuple[float, float]] = None
    if typing.get_origin(hint) is typing.Annotated:
        seen = set()
        for extra in hint.__metadata__:
            kind = type(extra)
            if kind in seen:
                continue
            if isinstance(extra, Unit):
                unit = extra.unit
            elif isinstance(extra, Description):
                description = extra.text
            elif isinstance(extra, Labels):
                labels = extra.labels
            elif isinstance(extra, Range):
                if extra.min is not None and extra.max is not None:
                    value_range = (extra.min, extra.max)
            else:
                continue
            seen.add(kind)
        hint = typing.get_args(hint)[0]
    # Optional[X] and X | None annotate X
    if typing.get_origin(hint) in (typing.Union, types.UnionType):
        args = [a for a in typing.get_args(hint) if a is not type(None)]
        if len(args) == 1:
            inner_meta, inner = _metadata_of(args[0])
            return FieldMetadata(
                unit or inner_meta.unit,
                description or inner_meta.description,
                labels or inner_meta.labels,
                value_range or inner_meta.value_range,
            ), inner
    return FieldMetadata(unit, description, labels, value_range), hint
