"""Actions a protocol offers to blip, e.g. to set a motor setpoint or a log level.

An action is a Python function. Its parameters are taken from its signature, so the
type hints decide how blip lets the user enter the arguments::

    class MyTarget(Protocol):
        @action("motor/speed", readback="motor.speed_rpm")
        async def set_speed(self, rpm: Annotated[int, Range(0, 3000), Unit("rpm")]) -> None:
            ...

Supported parameter types are ``bool``, ``int``, ``float``, ``str``, ``Literal`` of
strings and ``enum.Enum`` subclasses (offered by member name), each optionally wrapped
in ``Annotated`` with a ``Range`` and a ``Unit``. A default value of the parameter is
offered as the initial value in blip.
"""

import asyncio
import enum
import inspect
import logging
import typing
from dataclasses import dataclass, field
from typing import Any, Awaitable, Callable, Dict, List, Optional, Union

logger = logging.getLogger("bridge.actions")

# The attribute holding the ActionInfo of a function decorated with @action
_ACTION_ATTRIBUTE = "__blip_action__"


class ActionError(Exception):
    """Raised by an action to reject an invocation, the message is shown in blip."""


@dataclass(frozen=True)
class Range:
    """Bounds of a numeric parameter, and the step of a float parameter."""
    min: Optional[float] = None
    max: Optional[float] = None
    step: Optional[float] = None


@dataclass(frozen=True)
class Unit:
    """The unit of a parameter."""
    unit: str


@dataclass(frozen=True)
class _ActionInfo:
    path: str
    description: str
    readback: Optional[str]
    confirm: bool


def action(
    path: str,
    *,
    description: str = "",
    readback: Optional[str] = None,
    confirm: bool = False,
) -> Callable[[Callable], Callable]:
    """Marks a method of a protocol as an action.

    `path` names the action, levels separated by "/". The description defaults to the
    first line of the docstring. `readback` is the path of the trace that shows the
    current value of what the action sets. With `confirm`, blip asks the user before
    sending the action.
    """
    def decorate(function: Callable) -> Callable:
        setattr(function, _ACTION_ATTRIBUTE, _ActionInfo(path, description, readback, confirm))
        return function
    return decorate


def declared_actions(owner: object) -> List[tuple]:
    """The methods of `owner` decorated with @action, bound to it, with their info, in
    the order they are defined, base classes first."""
    members: Dict[str, Any] = {}
    for cls in reversed(type(owner).__mro__):
        members.update(vars(cls))
    found = []
    for name, member in members.items():
        info = getattr(member, _ACTION_ATTRIBUTE, None)
        if isinstance(info, _ActionInfo):
            found.append((info, getattr(owner, name)))
    return found


@dataclass
class Param:
    name: str
    # "Bool", "Int", "Float", "Enum" or "String"
    kind: str
    unit: str = ""
    min: Optional[float] = None
    max: Optional[float] = None
    step: Optional[float] = None
    options: List[str] = field(default_factory=list)
    # the Enum class whose members are offered by name
    enum_type: Optional[type] = None
    required: bool = True
    default: Any = None

    def to_dict(self) -> Dict[str, Any]:
        if self.kind == "Int":
            kind: Any = {"Int": {"min": self.min, "max": self.max}}
        elif self.kind == "Float":
            kind = {"Float": {"min": self.min, "max": self.max, "step": self.step}}
        elif self.kind == "Enum":
            kind = {"Enum": list(self.options)}
        else:
            kind = self.kind
        default = self.default
        if isinstance(default, enum.Enum):
            default = default.name
        return {
            "name": self.name,
            "kind": kind,
            "unit": self.unit,
            "default": default if not self.required else None,
        }

    def convert(self, value: Any) -> Any:
        """The argument for the function, raises ActionError for an invalid value."""
        if self.kind == "Bool":
            if not isinstance(value, bool):
                raise ActionError(f"{self.name}: expected a bool, got {value!r}")
            return value
        if self.kind in ("Int", "Float"):
            is_number = isinstance(value, (int, float)) and not isinstance(value, bool)
            if self.kind == "Int" and not (is_number and isinstance(value, int)):
                raise ActionError(f"{self.name}: expected an integer, got {value!r}")
            if not is_number:
                raise ActionError(f"{self.name}: expected a number, got {value!r}")
            if self.min is not None and value < self.min:
                raise ActionError(f"{self.name}: {value} is below the minimum {self.min}")
            if self.max is not None and value > self.max:
                raise ActionError(f"{self.name}: {value} is above the maximum {self.max}")
            return float(value) if self.kind == "Float" else value
        if not isinstance(value, str):
            raise ActionError(f"{self.name}: expected a string, got {value!r}")
        if self.kind == "Enum":
            if value not in self.options:
                raise ActionError(f"{self.name}: {value!r} is not one of {', '.join(self.options)}")
            return self.enum_type[value] if self.enum_type else value
        return value


def _param(name: str, annotation: Any, default: Any) -> Param:
    """The parameter of a function's parameter, raises TypeError if not supported."""
    metadata: tuple = ()
    if typing.get_origin(annotation) is typing.Annotated:
        annotation, *rest = typing.get_args(annotation)
        metadata = tuple(rest)
    ranges = [item for item in metadata if isinstance(item, Range)]
    units = [item for item in metadata if isinstance(item, Unit)]
    bounds = ranges[0] if ranges else Range()
    param = Param(name=name, kind="", unit=units[0].unit if units else "")
    if default is not inspect.Parameter.empty:
        param.required = False
        param.default = default

    if annotation is bool:
        param.kind = "Bool"
    elif annotation is int:
        param.kind = "Int"
        for bound in (bounds.min, bounds.max):
            if bound is not None and not isinstance(bound, int):
                raise TypeError(f"parameter '{name}': the range of an int needs int bounds")
        if bounds.step is not None:
            raise TypeError(f"parameter '{name}': an int has no step")
        param.min, param.max = bounds.min, bounds.max
    elif annotation is float:
        param.kind = "Float"
        param.min, param.max, param.step = bounds.min, bounds.max, bounds.step
    elif annotation is str:
        param.kind = "String"
    elif typing.get_origin(annotation) is typing.Literal:
        options = typing.get_args(annotation)
        if not all(isinstance(option, str) for option in options):
            raise TypeError(f"parameter '{name}': a Literal needs strings")
        param.kind = "Enum"
        param.options = list(options)
    elif isinstance(annotation, type) and issubclass(annotation, enum.Enum):
        param.kind = "Enum"
        param.options = [member.name for member in annotation]
        param.enum_type = annotation
    else:
        raise TypeError(f"parameter '{name}': unsupported type {annotation!r}")

    if param.kind not in ("Int", "Float") and (ranges or bounds != Range()):
        raise TypeError(f"parameter '{name}': only numbers have a range")
    return param


def _params(function: Callable) -> List[Param]:
    hints = typing.get_type_hints(function, include_extras=True)
    params = []
    for name, parameter in inspect.signature(function).parameters.items():
        if parameter.kind in (parameter.VAR_POSITIONAL, parameter.VAR_KEYWORD):
            raise TypeError(f"parameter '{name}': *args and **kwargs are not supported")
        if name not in hints:
            raise TypeError(f"parameter '{name}' has no type hint")
        params.append(_param(name, hints[name], parameter.default))
    return params


@dataclass
class _Action:
    path: str
    function: Callable[..., Union[None, Awaitable[None]]]
    params: List[Param]
    description: str
    readback: Optional[str]
    confirm: bool
    enabled: bool = True
    # invocations of an action run one after another, in the order they arrived
    lock: asyncio.Lock = field(default_factory=asyncio.Lock)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "path": self.path,
            "description": self.description,
            "params": [param.to_dict() for param in self.params],
            "readback": self.readback,
            "confirm": self.confirm,
            "enabled": self.enabled,
        }


class Actions:
    """The actions of a protocol. Changing them announces them to blip again."""

    def __init__(self, on_change: Callable[[], None] = lambda: None) -> None:
        self._actions: Dict[str, _Action] = {}
        # called after each change
        self.on_change = on_change

    def add(
        self,
        path: str,
        function: Callable,
        *,
        description: str = "",
        readback: Optional[str] = None,
        confirm: bool = False,
        enabled: bool = True,
    ) -> None:
        """Adds an action carried out by `function`, sync or async, replacing an action
        of the same path. Raises TypeError if the parameters of `function` are not
        supported."""
        if not path:
            raise ValueError("an action needs a path")
        if not description:
            description = (inspect.getdoc(function) or "").split("\n", 1)[0]
        self._actions[path] = _Action(
            path, function, _params(function), description, readback, confirm, enabled
        )
        self.on_change()

    def remove(self, path: str) -> None:
        if self._actions.pop(path, None) is not None:
            self.on_change()

    def clear(self) -> None:
        if self._actions:
            self._actions.clear()
            self.on_change()

    def set_enabled(self, enabled: bool, *paths: str) -> None:
        """Enables or disables the actions at `paths`, all actions if none are given."""
        changed = False
        for path in paths or list(self._actions):
            action = self._actions.get(path)
            if action is None:
                raise KeyError(f"no action '{path}'")
            changed |= action.enabled != enabled
            action.enabled = enabled
        if changed:
            self.on_change()

    def is_enabled(self, path: str) -> bool:
        return self._actions[path].enabled

    def __contains__(self, path: str) -> bool:
        return path in self._actions

    def __len__(self) -> int:
        return len(self._actions)

    def descriptors(self) -> List[Dict[str, Any]]:
        """The actions as sent to blip in an Actions message."""
        return [action.to_dict() for action in self._actions.values()]

    async def invoke(self, path: str, args: Any) -> None:
        """Carries out the action at `path` with the record `args`, raises ActionError
        if the action is unknown or disabled, the arguments are invalid or the action
        rejects them."""
        action = self._actions.get(path)
        if action is None:
            raise ActionError(f"unknown action '{path}'")
        if not action.enabled:
            raise ActionError(f"'{path}' is disabled")
        if not isinstance(args, dict):
            raise ActionError("the arguments are not a record")
        unknown = set(args) - {param.name for param in action.params}
        if unknown:
            raise ActionError(f"unknown arguments: {', '.join(sorted(unknown))}")
        kwargs = {}
        for param in action.params:
            if param.name in args:
                kwargs[param.name] = param.convert(args[param.name])
            elif param.required:
                raise ActionError(f"{param.name}: missing")

        async with action.lock:
            try:
                result = action.function(**kwargs)
                if inspect.isawaitable(result):
                    await result
            except ActionError:
                raise
            except Exception as e:
                logger.exception(f"Action '{path}' failed")
                raise ActionError(f"{type(e).__name__}: {e}") from e
