"""blip_outpost sends the traces, log records and actions of your target to blip.

Your script talks to the target and passes on what it decodes::

    import blip_outpost as outpost

    @outpost.action("motor/speed", readback="motor/speed_rpm")
    def set_speed(rpm: Annotated[int, outpost.Range(0, 3000)]) -> None:
        link.write_setpoint(rpm)

    outpost.start()                      # serves ws://127.0.0.1:3000/ws/outpost
    outpost.unit("motor/speed_rpm", "rpm")
    while True:
        frame = link.read()
        with outpost.batch(timestamp_ns=frame.t_ns):
            outpost.value("motor/speed_rpm", frame.speed)
            outpost.value("battery", {"voltage": frame.vbat, "current": frame.ibat})

A Python program that traces itself gets tracers by name, as loggers::

    motor = outpost.get_tracer("motor")
    motor.value("speed", 1432)           # the trace motor/speed
    motor.info("homed")                  # a log record at motor, of driver

The functions of this module use one outpost of the program, see `get_outpost`; create
an `Outpost` for another one, e.g. on another port or in asyncio code.
"""

from pathlib import Path
from typing import Any, Callable, ContextManager, Optional, Sequence, Union

from blip_outpost.actions import ActionError, Actions, Range, Unit
from blip_outpost.outpost import Outpost
from blip_outpost.proto import Data, LogLevel, Message
from blip_outpost.timing import sleep_until
from blip_outpost.tracer import LogHandler, Tracer

__version__ = "0.1.0"

__all__ = [
    "ActionError",
    "Actions",
    "Data",
    "LogHandler",
    "LogLevel",
    "Message",
    "Outpost",
    "Range",
    "Tracer",
    "Unit",
    "action",
    "batch",
    "drain",
    "get_outpost",
    "get_tracer",
    "log",
    "register",
    "send",
    "serve",
    "series",
    "sleep_until",
    "start",
    "stop",
    "unit",
    "value",
    "wait",
    "wait_for_client",
]

_outpost = Outpost()


def get_outpost() -> Outpost:
    """The outpost the functions of this module use, e.g. for its `actions`."""
    return _outpost


def _configure(host: str, port: int, path: str, record: Optional[Union[str, Path]]) -> Outpost:
    _outpost.host, _outpost.port, _outpost.path, _outpost.record = host, port, path, record
    return _outpost


def start(
    host: str = "127.0.0.1",
    port: int = 3000,
    path: str = "/ws/outpost",
    record: Optional[Union[str, Path]] = None,
) -> Outpost:
    """Serves blip on ws://host:port/path from a thread of its own, recording what is
    sent to the file `record` if given, and returns the outpost. ``host="0.0.0.0"``
    serves blips on other machines too."""
    return _configure(host, port, path, record).start()


def serve(
    host: str = "127.0.0.1",
    port: int = 3000,
    path: str = "/ws/outpost",
    record: Optional[Union[str, Path]] = None,
) -> Outpost:
    """`start` for asyncio code, serving from the running event loop.

    For example::

        async with blip_outpost.serve(port=3000):
            ...
    """
    return _configure(host, port, path, record)


def stop() -> None:
    """Stops serving, and finishes the recording."""
    _outpost.stop()


def wait() -> None:
    """Blocks until interrupted with Ctrl+C, then stops."""
    _outpost.wait()


def value(path: str, value: Data, timestamp_ns: Optional[int] = None) -> None:
    """Sends a sample of the trace `path`, timestamped now unless given (ns), see
    `Outpost.value`."""
    _outpost.value(path, value, timestamp_ns)


def series(
    path: str,
    values: Sequence[Data],
    *,
    timestamps_ns: Optional[Sequence[int]] = None,
    start_ns: Optional[int] = None,
    period_ns: Optional[float] = None,
) -> None:
    """Sends many samples of the trace `path` at once, see `Outpost.series`."""
    _outpost.series(path, values, timestamps_ns=timestamps_ns, start_ns=start_ns, period_ns=period_ns)


def unit(path: str, unit: str) -> None:
    """Sends the unit of the trace `path`; sent once, it reaches every blip."""
    _outpost.unit(path, unit)


def log(
    level: int,
    message: str,
    *,
    path: str = "",
    timestamp_ns: Optional[int] = None,
) -> None:
    """Sends a log record of a `LogLevel` at `path`, timestamped now
    unless given (ns), see `Outpost.log`."""
    _outpost.log(level, message, path=path, timestamp_ns=timestamp_ns)


def send(message: Union[Message, bytes]) -> None:
    """Sends a `Message`, or the bytes of complete messages as a target that speaks the
    blip protocol sends them."""
    _outpost.send(message)


def batch(timestamp_ns: Optional[int] = None) -> ContextManager[None]:
    """Hands what this thread sends in the block over at once, see `Outpost.batch`."""
    return _outpost.batch(timestamp_ns)


def get_tracer(name: str = "") -> Tracer:
    """The tracer `name` of the outpost of the module, a path such as ``motor``, whose
    log records are at its name; the same name gives the same tracer. See
    `Tracer`."""
    return _outpost.get_tracer(name)


def drain() -> None:
    """Blocks until every blip has room for more messages, see `Outpost.drain`."""
    _outpost.drain()


def wait_for_client(timeout: Optional[float] = None) -> bool:
    """Blocks until a blip is connected, at most `timeout` seconds; returns whether one
    is."""
    return _outpost.wait_for_client(timeout)


def action(
    path: str,
    *,
    description: str = "",
    readback: Optional[str] = None,
    confirm: bool = False,
) -> Callable[[Callable], Callable]:
    """Makes the decorated function an action at `path`, see `Outpost.action`. A method
    becomes an action of each object passed to `register`."""
    return _outpost.action(path, description=description, readback=readback, confirm=confirm)


def register(owner: Any) -> None:
    """Adds the methods of `owner` decorated with `action` as its actions."""
    _outpost.register(owner)
