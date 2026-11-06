"""Tracers: named like loggers, for tracing a Python program.

Get a tracer by its name, a path, anywhere in the program; the same name gives the same
tracer::

    motor = blip_outpost.get_tracer("motor", component="driver")
    motor.unit("speed", "rpm")
    motor.value("speed", 1432)              # the trace motor/speed
    motor.warn("hot: %d °C", temperature)   # a log record at motor, of driver

A tracer only adds its name to the paths and its component to the log records, and
sends with its outpost, see `Outpost.value`, `Outpost.log` and the others. Tracers may be
created and used before the outpost serves, e.g. when a module is imported.
"""

import logging
from typing import TYPE_CHECKING, Any, Callable, ContextManager, Optional, Sequence

from blip_outpost.proto import Data, LogLevel

if TYPE_CHECKING:
    from blip_outpost.outpost import Outpost


def join_path(prefix: str, path: str) -> str:
    """`path` below `prefix`, levels separated by "/"."""
    if not prefix:
        return path
    if not path:
        return prefix
    return f"{prefix}/{path}"


class Tracer:
    """Sends the traces and log records of a part of the program: its traces are below
    its `name`, its log records at `name` and of its `component`. Get one with
    `get_tracer` or `Outpost.get_tracer`. It may be used from any thread."""

    def __init__(self, outpost: "Outpost", name: str = "", component: str = "") -> None:
        self._outpost = outpost
        self._name = name
        self._component = component

    def __repr__(self) -> str:
        component = f" ({self._component})" if self._component else ""
        return f"<Tracer {self._name or '/'}{component}>"

    @property
    def name(self) -> str:
        """The path the traces of this tracer are below, e.g. ``motor``; empty for the
        target as a whole."""
        return self._name

    @property
    def component(self) -> str:
        """The component of the log records, the part of the software that writes them."""
        return self._component

    @property
    def outpost(self) -> "Outpost":
        return self._outpost

    def path(self, path: str = "") -> str:
        """The full path of the trace `path` of this tracer."""
        return join_path(self._name, path)

    def child(self, name: str, component: Optional[str] = None) -> "Tracer":
        """The tracer `name` below this one, e.g. ``motor/left`` for ``left``, with the
        component of this one unless given."""
        return self._outpost.get_tracer(
            join_path(self._name, name.strip("/")), self._component if component is None else component
        )

    # Traces

    def value(self, path: str, value: Data, timestamp_ns: Optional[int] = None) -> None:
        """Sends a sample of the trace `path` below this tracer, see `Outpost.value`."""
        self._outpost.value(join_path(self._name, path), value, timestamp_ns)

    def series(
        self,
        path: str,
        values: Sequence[Data],
        *,
        timestamps_ns: Optional[Sequence[int]] = None,
        start_ns: Optional[int] = None,
        period_ns: Optional[float] = None,
    ) -> None:
        """Sends many samples of the trace `path` below this tracer at once, see
        `Outpost.series`."""
        self._outpost.series(
            join_path(self._name, path), values, timestamps_ns=timestamps_ns, start_ns=start_ns, period_ns=period_ns
        )

    def unit(self, path: str, unit: str) -> None:
        """Sends the unit of the trace `path` below this tracer, see `Outpost.unit`."""
        self._outpost.unit(join_path(self._name, path), unit)

    def batch(self, timestamp_ns: Optional[int] = None) -> ContextManager[None]:
        """Hands what this thread sends in the block over at once, see `Outpost.batch`."""
        return self._outpost.batch(timestamp_ns)

    # Log records

    def log(self, level: int, message: str, *args: Any, timestamp_ns: Optional[int] = None) -> None:
        """Sends a log record of a `LogLevel` at the path of this tracer and of its
        component. With `args`, the message is formatted with them as `logging` does,
        ``message % args``, e.g. ``log(LogLevel.WARN, "hot: %d °C", t)``."""
        if args:
            message = message % args
        self._outpost.log(level, message, path=self._name, component=self._component, timestamp_ns=timestamp_ns)

    def debug(self, message: str, *args: Any, timestamp_ns: Optional[int] = None) -> None:
        self.log(LogLevel.DEBUG, message, *args, timestamp_ns=timestamp_ns)

    def info(self, message: str, *args: Any, timestamp_ns: Optional[int] = None) -> None:
        self.log(LogLevel.INFO, message, *args, timestamp_ns=timestamp_ns)

    def warn(self, message: str, *args: Any, timestamp_ns: Optional[int] = None) -> None:
        self.log(LogLevel.WARN, message, *args, timestamp_ns=timestamp_ns)

    # as in logging
    warning = warn

    def error(self, message: str, *args: Any, timestamp_ns: Optional[int] = None) -> None:
        self.log(LogLevel.ERROR, message, *args, timestamp_ns=timestamp_ns)

    # Actions

    def action(
        self,
        path: str,
        *,
        description: str = "",
        readback: Optional[str] = None,
        confirm: bool = False,
    ) -> Callable[[Callable], Callable]:
        """Makes the decorated function the action `path` below this tracer, with the
        readback trace `readback` below it too, see `Outpost.action`."""
        return self._outpost.action(
            join_path(self._name, path),
            description=description,
            readback=join_path(self._name, readback) if readback is not None else None,
            confirm=confirm,
        )


# The levels of `logging` and the ones of blip they become, the highest first
_LEVELS = (
    (logging.ERROR, LogLevel.ERROR),
    (logging.WARNING, LogLevel.WARN),
    (logging.INFO, LogLevel.INFO),
    (logging.DEBUG, LogLevel.DEBUG),
)

# Loggers whose records are not sent: sending them could log again
_OWN_LOGGERS = ("blip_outpost", "websockets", "asyncio")


def blip_level(levelno: int) -> LogLevel:
    """The level of blip of a level of `logging`; below DEBUG it is TRACE."""
    for threshold, level in _LEVELS:
        if levelno >= threshold:
            return level
    return LogLevel.TRACE


class LogHandler(logging.Handler):
    """Sends the records of Python's `logging` to blip, with the name of the logger as
    the component, at `path`, or of the target as a whole without one::

        logging.getLogger().addHandler(blip_outpost.LogHandler())

    The records of the outpost itself and of the libraries it uses (``blip_outpost``,
    ``websockets``, ``asyncio``) are not sent. Sends with the outpost of the module
    unless given."""

    def __init__(self, path: str = "", outpost: Optional["Outpost"] = None, level: int = logging.NOTSET) -> None:
        super().__init__(level)
        self.path = path
        self._outpost = outpost

    def emit(self, record: logging.LogRecord) -> None:
        name = record.name
        if any(name == own or name.startswith(own + ".") for own in _OWN_LOGGERS):
            return
        try:
            outpost = self._outpost
            if outpost is None:
                from blip_outpost import get_outpost

                outpost = get_outpost()
            outpost.log(
                blip_level(record.levelno),
                self.format(record),
                path=self.path,
                component="" if name == "root" else name,
            )
        except Exception:
            self.handleError(record)
