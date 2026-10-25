"""The outpost: serves blip what your script sends, and calls your actions.

Your script owns the link to the target. It starts the outpost, which serves blip from
a thread of its own, and sends what it decodes from the target, from any thread::

    outpost = Outpost().start()
    outpost.send_metadata("motor/speed_rpm", "rpm")
    while True:
        outpost.send_value("motor/speed_rpm", read_speed())

In asyncio code the outpost serves from the running event loop instead::

    async with Outpost() as outpost:
        ...
"""

import asyncio
import atexit
import collections
import logging
import threading
from pathlib import Path
from typing import Any, Awaitable, Callable, Deque, List, Optional, Union

from blip_outpost.actions import Actions, declared_actions, is_method, mark_action
from blip_outpost.proto import Data, Message, split_items
from blip_outpost.recording import RecordingWriter
from blip_outpost.server import OutpostServer

logger = logging.getLogger(__name__)

# Carries out an invocation of blip in place of the actions: the path and the arguments
InvokeHandler = Callable[[str, Any], Awaitable[None]]


class Outpost:
    """Serves blip on ws://host:port/path: what is sent goes to every connected blip
    and to the recording file `record`, if given, and blip's invocations call the
    actions.

    `start` serves from a thread of its own, `async with` from the running event loop.
    The `send_*` methods may be called from any thread, before, while and after the
    outpost serves; they never block. Units and actions are kept and sent to each blip
    when it connects, so they may be sent once at any time. Other messages are passed
    on while the outpost serves, and dropped otherwise.

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

        self._server = OutpostServer(self._invoke, max_queued=max_queued, on_clients=self._clients_changed)
        # the event loop serving, and its thread, while the outpost serves
        self._loop: Optional[asyncio.AbstractEventLoop] = None
        self._loop_thread: Optional[int] = None
        # the thread of `start`, and the event that ends it
        self._thread: Optional[threading.Thread] = None
        self._stop: Optional[asyncio.Event] = None
        # messages sent from other threads, passed to the server in its event loop
        self._pending: Deque[bytes] = collections.deque()
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

    def send_value(self, path: str, value: Data, timestamp_ns: Optional[int] = None) -> None:
        """Sends a sample of the trace `path`, timestamped now unless given (ns).

        `value` is None, int, float, bool, str, a list of fixed length or a dict with
        str keys, nested freely. Python ints are sent as integers, so pass a float for
        a float trace."""
        self._send_frame(Message.create_value(path, value, timestamp_ns).encode_cbor())

    def send_metadata(self, path: str, unit: str) -> None:
        """Sends the unit of the trace `path`; for a field of a record, the path joined
        with ".", e.g. ``imu.acc``. Sent once, it reaches every blip."""
        self._send_frame(Message.create_metadata(path, unit).encode_cbor())

    def send_log(
        self,
        level: int,
        message: str,
        component: str = "",
        timestamp_ns: Optional[int] = None,
    ) -> None:
        """Sends a log record of a `LogLevel`, timestamped now unless given (ns)."""
        self._send_frame(Message.create_log(int(level), message, timestamp_ns, component).encode_cbor())

    def send(self, message: Union[Message, bytes]) -> None:
        """Sends a `Message`, or the bytes of complete messages: one or several one
        after another, a CBOR sequence (RFC 8742), as a target that speaks the blip
        protocol sends them. Bytes that are no messages are dropped with an error."""
        if isinstance(message, Message):
            self._send_frame(message.encode_cbor())
            return
        try:
            frames, rest = split_items(message)
        except ValueError as e:
            logger.error(f"Dropping {len(message)} bytes that are no messages ({e}): {message[:32].hex()}")
            return
        if rest:
            logger.error(f"Dropping {len(rest)} bytes of an incomplete message: {rest[:32].hex()}")
        for frame in frames:
            self._send_frame(frame)

    def _send_frame(self, frame: bytes) -> None:
        """Sends the bytes of a single message, unchecked."""
        loop = self._loop
        if loop is None or threading.get_ident() == self._loop_thread:
            self._server.broadcast(frame)
            return
        # from another thread: passed on in the event loop, many at once
        self._pending.append(frame)
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
        while pending:
            self._server.broadcast(pending.popleft())

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
