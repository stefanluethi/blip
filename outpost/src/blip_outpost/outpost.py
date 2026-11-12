"""The outpost: serves blip what your script sends, and calls your actions.

Your script owns the link to the target. It starts the outpost, which serves blip from
a thread of its own, and sends what it decodes from the target, from any thread::

    outpost = Outpost().start()
    outpost.unit("motor/speed_rpm", "rpm")
    while True:
        outpost.value("motor/speed_rpm", read_speed())

A program that traces itself gets named tracers instead, see `Outpost.get_tracer`.

In asyncio code the outpost serves from the running event loop instead::

    async with Outpost() as outpost:
        ...
"""

import asyncio
import atexit
import collections
import contextlib
import dataclasses
import logging
import threading
import time
from pathlib import Path
from typing import Any, Awaitable, Callable, Deque, Dict, Iterator, List, Optional, Sequence, Set, Tuple, Union

from blip_outpost.actions import Actions, declared_actions, is_method, mark_action
from blip_outpost.proto import Data, Message, encode_value, frames_of
from blip_outpost.records import field_units
from blip_outpost.recording import RecordingWriter
from blip_outpost.server import OutpostServer
from blip_outpost.tracer import Tracer

logger = logging.getLogger(__name__)

# Carries out an invocation of blip in place of the actions: the path and the arguments
InvokeHandler = Callable[[str, Any], Awaitable[None]]

# The types of values that are no dataclass, for a quick check
_PLAIN_TYPES = frozenset((int, float, bool, str, type(None), list, dict))


def _as_list(values: Any) -> list:
    """`values` as a list, e.g. of a numpy array."""
    tolist = getattr(values, "tolist", None)
    return tolist() if callable(tolist) else list(values)


class Outpost:
    """Serves blip on ws://host:port/path: what is sent goes to every connected blip
    and to the recording file `record`, if given, and blip's invocations call the
    actions.

    `start` serves from a thread of its own, `async with` from the running event loop.
    `value`, `series`, `unit`, `log` and `send` may be called from any thread, before,
    while and after the outpost serves; they never block. Units and actions are kept
    and sent to each blip when it connects, so they may be sent once at any time. Other
    messages are passed on while the outpost serves, and dropped otherwise.

    Messages for a blip that does not keep up are dropped, with a warning, once more
    than `max_queued` are waiting for it; see `drain` to wait for it instead.
    """

    def __init__(
        self,
        host: str = "127.0.0.1",
        port: int = 3000,
        path: str = "/ws/outpost",
        record: Optional[Union[str, Path]] = None,
        max_queued: int = 10000,
    ) -> None:
        self.host = host
        self.port = port
        self.path = path
        self.record = record
        # the actions offered to blip, changing them announces them again
        self.actions = Actions(on_change=self._announce_actions)
        # if set, carries out the invocations in place of the actions, e.g. to forward
        # them to a target that knows its actions itself
        self.on_invoke: Optional[InvokeHandler] = None
        # if set, the time of what is sent without a timestamp, e.g. the time of the
        # target a link translates, None while it is not known; else the time of this
        # computer
        self.target_time: Optional[Callable[[], Optional[int]]] = None
        self._warned_untimed = False

        self._server = OutpostServer(self._invoke, max_queued=max_queued, on_clients=self._clients_changed)
        # the event loop serving, and its thread, while the outpost serves
        self._loop: Optional[asyncio.AbstractEventLoop] = None
        self._loop_thread: Optional[int] = None
        # the thread of `start`, and the event that ends it
        self._thread: Optional[threading.Thread] = None
        self._stop: Optional[asyncio.Event] = None
        # messages sent from other threads, passed to the server in its event loop: a
        # single one, or the list of a batch
        self._pending: Deque[Union[bytes, List[bytes]]] = collections.deque()
        # the open batches of each thread: their timestamps and the messages sent
        self._local = threading.local()
        # the paths and dataclasses whose units are sent
        self._record_units: Set[Tuple[str, type]] = set()
        self._tracers: Dict[str, Tracer] = {}
        self._tracers_lock = threading.Lock()
        self._flush_scheduled = False
        # set while a blip is connected
        self._has_client = threading.Event()
        self._client_event: Optional[asyncio.Event] = None

    def __repr__(self) -> str:
        return f"<Outpost {self.url}{' serving' if self.serving else ''}>"

    @property
    def url(self) -> str:
        """The address blip connects to."""
        return f"ws://{self.host}:{self.port}{self.path}"

    @property
    def serving(self) -> bool:
        return self._loop is not None

    @property
    def clients(self) -> int:
        """The number of blips connected."""
        return len(self._server.connected_clients)

    # Serving

    def start(self) -> "Outpost":
        """Serves blip from a thread of its own until `stop`, or until the program
        exits. Returns the outpost; raises if it cannot serve, e.g. as the port is in
        use."""
        if self._thread is not None or self._loop is not None:
            raise RuntimeError("the outpost serves already")
        ready = threading.Event()
        failure: List[BaseException] = []

        def serve() -> None:
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            try:
                self._stop = asyncio.Event()
                loop.run_until_complete(self._open())
            except BaseException as e:
                failure.append(e)
                loop.close()
                ready.set()
                return
            ready.set()
            try:
                loop.run_until_complete(self._stop.wait())
            finally:
                loop.run_until_complete(self._close())
                loop.close()

        self._thread = threading.Thread(target=serve, name="blip-outpost", daemon=True)
        self._thread.start()
        ready.wait()
        if failure:
            self._thread = None
            raise failure[0]
        # so the recording is complete also when the script just ends
        atexit.register(self.stop)
        return self

    def stop(self) -> None:
        """Stops serving that `start` began, and finishes the recording."""
        thread = self._thread
        if thread is None:
            return
        loop, stop = self._loop, self._stop
        if loop is not None and stop is not None:
            try:
                loop.call_soon_threadsafe(stop.set)
            except RuntimeError:
                pass  # closed already
        if threading.current_thread() is not thread:
            thread.join()
        self._thread = None
        atexit.unregister(self.stop)

    def wait(self) -> None:
        """Blocks until interrupted with Ctrl+C or stopped from another thread, then
        stops. For a script that only offers actions, or sends from other threads."""
        if self._thread is None:
            raise RuntimeError("start() the outpost first")
        try:
            while self._thread is not None and self._thread.is_alive():
                # with a timeout, so Ctrl+C is noticed on Windows too
                self._thread.join(0.2)
        except KeyboardInterrupt:
            pass
        finally:
            self.stop()

    def __enter__(self) -> "Outpost":
        return self.start()

    def __exit__(self, *exc: Any) -> None:
        self.stop()

    async def __aenter__(self) -> "Outpost":
        if self._loop is not None:
            raise RuntimeError("the outpost serves already")
        await self._open()
        return self

    async def __aexit__(self, *exc: Any) -> None:
        await self._close()

    async def _open(self) -> None:
        recorder = RecordingWriter(self.record) if self.record else None
        try:
            await self._server.start(self.host, self.port, self.path, recorder)
        except BaseException:
            if recorder:
                recorder.close()
            raise
        self._client_event = asyncio.Event()
        self._loop = asyncio.get_running_loop()
        self._loop_thread = threading.get_ident()
        logger.info(f"Serving blip on {self.url}")

    async def _close(self) -> None:
        self._flush()
        await self._server.stop()
        self._loop = None
        self._loop_thread = None
        self._client_event = None
        self._has_client.clear()

    # Sending

    def value(self, path: str, value: Data, timestamp_ns: Optional[int] = None) -> None:
        """Sends a sample of the trace `path`, levels separated by "/", timestamped now
        unless given (ns), see `target_time` and `batch`.

        `value` is None, int, float, bool, str, a list of fixed length or a record: a
        dict with str keys or a dataclass, nested freely. Each field of a record is a
        trace of its own, its path joined with ".", e.g. ``imu.acc``; the units that the
        fields of a dataclass declare with `Unit` are sent with its first sample at
        `path`. Python ints are sent as integers, so pass a float for a float trace."""
        timestamp_ns = self._resolve(timestamp_ns)
        if timestamp_ns is None:
            return
        if type(value) not in _PLAIN_TYPES:
            self._announce_record_units(path, value)
        self._send_frame(encode_value(timestamp_ns, path, value))

    def series(
        self,
        path: str,
        values: Sequence[Data],
        *,
        timestamps_ns: Optional[Sequence[int]] = None,
        start_ns: Optional[int] = None,
        period_ns: Optional[float] = None,
    ) -> None:
        """Sends the samples `values` of the trace `path` at once, e.g. a block read
        from an ADC: at `timestamps_ns`, one for each, or the first at `start_ns` and
        then one every `period_ns`. `values` and `timestamps_ns` are sequences, e.g.
        lists or numpy arrays; a sample is what `value` sends."""
        values = _as_list(values)
        if timestamps_ns is not None:
            if start_ns is not None or period_ns is not None:
                raise TypeError("give timestamps_ns, or start_ns and period_ns, not both")
            times = _as_list(timestamps_ns)
            if len(times) != len(values):
                raise ValueError(f"{len(values)} values but {len(times)} timestamps")
        elif start_ns is not None and period_ns is not None:
            times = [start_ns + int(round(i * period_ns)) for i in range(len(values))]
        else:
            raise TypeError("give timestamps_ns, or start_ns and period_ns")
        for cls in {type(value) for value in values} - _PLAIN_TYPES:
            self._announce_record_units(path, next(v for v in values if type(v) is cls))
        self._send_frames([encode_value(int(t), path, value) for t, value in zip(times, values)])

    def unit(self, path: str, unit: str) -> None:
        """Sends the unit of the trace `path`; for a field of a record, the path joined
        with ".", e.g. ``imu.acc``. Sent once, it reaches every blip."""
        self._send_frame(Message.create_metadata(path, unit).encode_cbor())

    def log(
        self,
        level: int,
        message: str,
        *,
        path: str = "",
        timestamp_ns: Optional[int] = None,
    ) -> None:
        """Sends a log record of a `LogLevel`, or its number, timestamped now unless
        given (ns), see `target_time` and `batch`: at `path` like a trace, e.g.
        ``motor``, or of the target as a whole without one. The message is sent as it
        is."""
        timestamp_ns = self._resolve(timestamp_ns)
        if timestamp_ns is None:
            return
        self._send_frame(Message.create_log(int(level), message, timestamp_ns, path).encode_cbor())

    def send(self, message: Union[Message, bytes]) -> None:
        """Sends a `Message`, or the bytes of complete messages: one or several one
        after another, a CBOR sequence (RFC 8742), as a target that speaks the blip
        protocol sends them. Bytes that are no messages are dropped with an error."""
        if isinstance(message, Message):
            self._send_frame(message.encode_cbor())
            return
        self._send_frames(frames_of(message))

    @contextlib.contextmanager
    def batch(self, timestamp_ns: Optional[int] = None) -> Iterator[None]:
        """Hands what this thread sends in the block over at once, at its end, e.g. the
        samples of one frame of the target::

            with outpost.batch(timestamp_ns=frame.t_ns):
                outpost.value("motor/speed", frame.speed)
                outpost.value("battery/voltage", frame.vbat)

        What is sent without a timestamp in the block shares one: `timestamp_ns`, or
        the time it is first needed. Batches nest, the outermost hands over. What was
        sent is handed over also when the block raises. A batch belongs to its thread,
        so in asyncio code the block should not await: what other tasks send meanwhile
        would join it."""
        local = self._local
        # the timestamp of each open batch, in a list of one: a nested batch without a
        # timestamp shares the list of the batch around it, also when it is set later
        stack: Optional[List[List[Optional[int]]]] = getattr(local, "stack", None)
        if stack is None:
            stack = local.stack = []
            local.frames = []
        stack.append([timestamp_ns] if timestamp_ns is not None or not stack else stack[-1])
        try:
            yield
        finally:
            stack.pop()
            if not stack:
                frames, local.frames = local.frames, []
                self._send_frames(frames)

    def _resolve(self, timestamp_ns: Optional[int]) -> Optional[int]:
        """The timestamp of what is sent with `timestamp_ns`: it, the one of the open
        batch, or now; `None` to drop it, while the time of the target is not known."""
        if timestamp_ns is not None:
            return timestamp_ns
        stack = getattr(self._local, "stack", None)
        if stack:
            shared = stack[-1]
            if shared[0] is None:
                shared[0] = self._now()
            return shared[0]
        return self._now()

    def _now(self) -> Optional[int]:
        if self.target_time is None:
            return time.time_ns()
        return self._target_now()

    def _target_now(self) -> Optional[int]:
        """The time of `target_time` now, `None` with a warning, once, while it is not
        known: the time of this computer would be far from it."""
        assert self.target_time is not None
        t = self.target_time()
        if t is None and not self._warned_untimed:
            self._warned_untimed = True
            logger.warning("Dropping what is sent without a timestamp before the first timestamp of the target")
        return t

    def _announce_record_units(self, path: str, value: Any) -> None:
        """Sends the units of the fields of the dataclass `value` with its first sample
        at `path`."""
        cls = type(value)
        if not dataclasses.is_dataclass(cls) or (path, cls) in self._record_units:
            return
        self._record_units.add((path, cls))
        for field, unit in field_units(cls):
            self.unit(f"{path}.{field}", unit)

    def _send_frame(self, frame: bytes) -> None:
        """Sends the bytes of a single message, unchecked."""
        if getattr(self._local, "stack", None):
            self._local.frames.append(frame)
            return
        loop = self._loop
        if loop is None or threading.get_ident() == self._loop_thread:
            self._server.broadcast(frame)
            return
        # from another thread: passed on in the event loop, many at once
        self._pending.append(frame)
        self._schedule_flush(loop)

    def _send_frames(self, frames: List[bytes]) -> None:
        """Sends the bytes of several messages, handed over together."""
        if not frames:
            return
        if getattr(self._local, "stack", None):
            self._local.frames.extend(frames)
            return
        loop = self._loop
        if loop is None or threading.get_ident() == self._loop_thread:
            for frame in frames:
                self._server.broadcast(frame)
            return
        self._pending.append(frames)
        self._schedule_flush(loop)

    def _schedule_flush(self, loop: asyncio.AbstractEventLoop) -> None:
        if not self._flush_scheduled:
            self._flush_scheduled = True
            try:
                loop.call_soon_threadsafe(self._flush)
            except RuntimeError:
                pass  # the outpost stopped just now

    def _flush(self) -> None:
        # cleared first, so a message added meanwhile schedules another flush
        self._flush_scheduled = False
        pending = self._pending
        broadcast = self._server.broadcast
        while pending:
            item = pending.popleft()
            if isinstance(item, bytes):
                broadcast(item)
            else:
                for frame in item:
                    broadcast(frame)

    def drain(self) -> None:
        """Blocks until every blip has room for more messages. A script sending faster
        than blip takes them, e.g. data of the past, calls it every thousand messages
        or so to slow down to the slowest blip instead of losing messages for it.
        Returns right away while no blip is connected."""
        loop = self._loop
        if loop is None:
            return
        if threading.get_ident() == self._loop_thread:
            raise RuntimeError("in the event loop of the outpost, await drain_async()")
        asyncio.run_coroutine_threadsafe(self.drain_async(), loop).result()

    async def drain_async(self) -> None:
        """`drain` for a coroutine in the event loop of the outpost."""
        self._flush()
        await self._server.drain()

    def wait_for_client(self, timeout: Optional[float] = None) -> bool:
        """Blocks until a blip is connected, at most `timeout` seconds; returns whether
        one is."""
        if self._loop is not None and threading.get_ident() == self._loop_thread:
            raise RuntimeError("in the event loop of the outpost, await wait_for_client_async()")
        return self._has_client.wait(timeout)

    async def wait_for_client_async(self) -> None:
        """`wait_for_client` for a coroutine in the event loop of the outpost."""
        if self._client_event is None:
            raise RuntimeError("the outpost does not serve")
        await self._client_event.wait()

    def _clients_changed(self, count: int) -> None:
        if count:
            self._has_client.set()
            if self._client_event is not None:
                self._client_event.set()
        else:
            self._has_client.clear()
            if self._client_event is not None:
                self._client_event.clear()

    # Tracers

    def get_tracer(self, name: str = "") -> "Tracer":
        """The tracer `name` of this outpost, a path such as ``motor`` or
        ``robot/arm``, whose log records are at its name: the same tracer for the
        same name, from anywhere in the program. See `Tracer`."""
        name = name.strip("/")
        with self._tracers_lock:
            tracer = self._tracers.get(name)
            if tracer is None:
                tracer = self._tracers[name] = Tracer(self, name)
        return tracer

    # Actions

    def action(
        self,
        path: str,
        *,
        description: str = "",
        readback: Optional[str] = None,
        confirm: bool = False,
    ) -> Callable[[Callable], Callable]:
        """Makes the decorated function an action at `path`, levels separated by "/".

        A function, sync or async, becomes an action of this outpost right away. A
        method becomes an action of each object passed to `register`.

        The description defaults to the first line of the docstring. `readback` is the
        path of the trace that shows the current value of what the action sets. With
        `confirm`, blip asks the user before sending the action. An action succeeds when
        it returns and is rejected when it raises, `ActionError` with the reason shown
        in blip."""
        def decorate(function: Callable) -> Callable:
            mark_action(function, path, description, readback, confirm)
            if not is_method(function):
                self.actions.add(
                    path, function, description=description, readback=readback, confirm=confirm
                )
            return function
        return decorate

    def register(self, owner: object) -> None:
        """Adds the methods of `owner` decorated with `action` as its actions, in the
        order they are defined, and announces them once."""
        with self.actions.batch():
            for info, method in declared_actions(owner):
                self.actions.add(
                    info.path,
                    method,
                    description=info.description,
                    readback=info.readback,
                    confirm=info.confirm,
                )

    async def _invoke(self, path: str, args: Any) -> None:
        if self.on_invoke is not None:
            await self.on_invoke(path, args)
        else:
            await self.actions.invoke(path, args)

    def _announce_actions(self) -> None:
        self._send_frame(Message.create_actions(self.actions.descriptors()).encode_cbor())
