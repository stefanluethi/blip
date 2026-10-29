"""The WebSocket server sending to the connected blips and passing on their invocations."""

import asyncio
import logging
from typing import Any, Awaitable, Callable, Dict, List, Optional, Set
from urllib.parse import parse_qs, urlsplit

import websockets
from websockets.asyncio.server import Server, ServerConnection, serve

import cbor2

from blip_outpost.actions import ActionError
from blip_outpost.proto import (
    ACTION_RESULT_PREFIX,
    ACTIONS_PREFIX,
    METADATA_PREFIX,
    Message,
    decode_request,
)
from blip_outpost.recording import RecordingWriter

logger = logging.getLogger(__name__)

# The messages queued for a client are sent together in one WebSocket frame, as a CBOR
# sequence, up to this many bytes, so that a busy target does not cost a frame per
# message, neither here nor in blip. A client that reads one message per frame asks
# for that with `?batch=0` after the path.
MAX_BATCH_BYTES = 64 * 1024


def describe(path: str, args: Any) -> str:
    """An invocation as shown in the console log, e.g. ``motor/speed(rpm=1500)``."""
    if isinstance(args, dict):
        arguments = ", ".join(f"{name}={value!r}" for name, value in args.items())
    else:
        arguments = repr(args)
    return f"{path}({arguments})"


class _Client:
    """A connected blip client with its own queue of messages to send, so a slow
    client neither holds up the protocol nor the other clients. All messages to the
    client go through the queue, so they are sent in the order they were queued, those
    queued together in one frame."""

    def __init__(self, websocket: ServerConnection, max_queued: int, batched: bool = True) -> None:
        self.websocket = websocket
        self.queue: asyncio.Queue[bytes] = asyncio.Queue(maxsize=max_queued)
        # the bytes of messages sent together in a frame at most, 0 for one per frame
        self.max_batch_bytes = MAX_BATCH_BYTES if batched else 0
        # the messages dropped since the last one sent
        self.dropped = 0
        # set while the queue is at most half full or the client is gone, see `drain`
        self.room = asyncio.Event()
        self.room.set()
        self.closed = False

    def _has_room(self) -> bool:
        return self.closed or self.queue.maxsize <= 0 or self.queue.qsize() <= self.queue.maxsize // 2

    async def drain(self) -> None:
        """Waits until the queue is at most half full, or the client is gone."""
        while not self._has_room():
            self.room.clear()
            await self.room.wait()

    def close(self) -> None:
        """Marks the client as gone, which ends `drain`."""
        self.closed = True
        self.room.set()

    def enqueue(self, message: bytes) -> None:
        """Queues `message`, or drops it if the client does not keep up."""
        try:
            self.queue.put_nowait(message)
        except asyncio.QueueFull:
            if self.dropped == 0:
                logger.warning(f"Client {self.websocket.remote_address} does not keep up, dropping messages")
            self.dropped += 1

    async def send_queued(self) -> None:
        """Sends the queued messages until the connection is closed, as many in a frame
        as are queued, up to `max_batch_bytes`, a larger message in a frame of its own."""
        held: Optional[bytes] = None
        while True:
            batch = [held if held is not None else await self.queue.get()]
            held = None
            size = len(batch[0])
            while not self.queue.empty():
                message = self.queue.get_nowait()
                if size + len(message) > self.max_batch_bytes:
                    held = message
                    break
                batch.append(message)
                size += len(message)
            if self._has_room():
                self.room.set()
            # joined once, as adding to bytes copies all before each time
            frame = b"".join(batch)
            if self.dropped:
                logger.warning(f"Dropped {self.dropped} messages for client {self.websocket.remote_address}")
                self.dropped = 0
            await self.websocket.send(frame)


class OutpostServer:
    """WebSocket server sending each message to all blip clients and passing the
    invocations of each client to `invoke`.

    It keeps the latest list of actions and the latest unit of each trace, and sends
    them to each client when it connects and to a recording when it starts, so they
    may be sent once at any time, also before the server starts."""

    def __init__(
        self,
        invoke: Callable[[str, Any], Awaitable[None]],
        max_queued: int = 10000,
        on_clients: Callable[[int], None] = lambda count: None,
    ) -> None:
        # carries out an invocation, raises ActionError to reject it
        self.invoke = invoke
        # called with the number of clients when a client connects or disconnects
        self.on_clients = on_clients
        # records the binary messages, also while no client is connected
        self.recorder: Optional[RecordingWriter] = None
        self.path = ""
        # the messages queued for a client at most, more are dropped
        self.max_queued = max_queued

        self.connected_clients: Set[_Client] = set()
        self._server: Optional[Server] = None
        self._invocations: Set[asyncio.Task] = set()
        self._actions_frame: Optional[bytes] = None
        # the latest ValueMetadata message of each path
        self._metadata: Dict[str, bytes] = {}

    def cached_frames(self) -> List[bytes]:
        """The messages a client gets when it connects: the units and the actions."""
        frames = list(self._metadata.values())
        if self._actions_frame is not None:
            frames.append(self._actions_frame)
        return frames

    async def drain(self) -> None:
        """Waits until every client has room for more messages: its queue is at most
        half full. Returns right away while no client is connected."""
        for client in list(self.connected_clients):
            await client.drain()

    def broadcast(self, message: bytes) -> None:
        """Queues a single message for all clients and records it, but answers to
        invocations: the server answers them itself. Called in the event loop of the
        server, or while it is not running, when it only keeps the units and the
        actions."""
        if message.startswith(ACTION_RESULT_PREFIX):
            # e.g. of a target that knows the actions itself, or of a replay: the outpost
            # answers the invoking client itself, and these ids are not that client's
            logger.debug("Dropping an ActionResult")
            return
        if message.startswith(ACTIONS_PREFIX):
            self._actions_frame = message
        elif message.startswith(METADATA_PREFIX):
            self._remember_metadata(message)
        if self.recorder:
            self.recorder.write(message)
        for client in self.connected_clients:
            client.enqueue(message)

    def _remember_metadata(self, message: bytes) -> None:
        try:
            path = cbor2.loads(message)["ValueMetadata"]["path"]
        except Exception:
            return
        if isinstance(path, str):
            # moved to the end, so a client gets the units in the order they were last sent
            self._metadata.pop(path, None)
            self._metadata[path] = message

    async def _handle_connection(self, websocket: ServerConnection) -> None:
        """Handle individual WebSocket client connection."""
        path = websocket.request.path if websocket.request else ""
        url = urlsplit(path)
        req_path = url.path.rstrip("/")
        batched = parse_qs(url.query).get("batch", ["1"])[-1] != "0"
        # Allow connecting to target path or wildcard root
        if self.path and req_path != self.path and req_path != "":
            logger.warning(f"Connection rejected: Path '{req_path}' does not match expected '{self.path}'")
            await websocket.close(code=1008, reason="Invalid path")
            return

        client_addr = websocket.remote_address
        logger.info(f"blip connected from {client_addr}")
        # added and given the cached messages without awaiting in between, so a change
        # announced later is queued after what it replaces
        client = _Client(websocket, self.max_queued, batched)
        self.connected_clients.add(client)
        for frame in self.cached_frames():
            client.enqueue(frame)
        sender = asyncio.create_task(client.send_queued())
        self.on_clients(len(self.connected_clients))

        try:
            async for message in websocket:
                if isinstance(message, str):
                    logger.warning(f"Ignoring a text message from {client_addr}")
                    continue
                self._start_invocation(client, message)
        except websockets.ConnectionClosedOK:
            logger.info(f"blip {client_addr} disconnected")
        except websockets.ConnectionClosedError as e:
            logger.warning(f"blip {client_addr} disconnected with an error: {e}")
        except Exception as e:
            logger.error(f"Error handling blip {client_addr}: {e}")
        finally:
            self.connected_clients.discard(client)
            client.close()
            sender.cancel()
            try:
                await sender
            except (asyncio.CancelledError, websockets.ConnectionClosed):
                pass
            except Exception as e:
                logger.error(f"Error sending to blip {client_addr}: {e}")
            self.on_clients(len(self.connected_clients))

    def _start_invocation(self, client: _Client, frame: bytes) -> None:
        try:
            invocation = decode_request(frame)
        except ValueError as e:
            logger.warning(f"Ignoring a message from {client.websocket.remote_address}: {e}")
            return
        # in a task, so a slow action does not hold up the invocations of other actions
        task = asyncio.create_task(self._invoke(client, invocation.id, invocation.path, invocation.args))
        self._invocations.add(task)
        task.add_done_callback(self._invocations.discard)

    async def _invoke(self, client: _Client, id: int, path: str, args: Any) -> None:
        error: Optional[str] = None
        try:
            await self.invoke(path, args)
        except ActionError as e:
            error = str(e) or "rejected"
        except Exception as e:
            logger.exception(f"Invoking '{path}' failed")
            error = f"{type(e).__name__}: {e}"

        # in the client's queue, so it stays in order with the broadcast messages
        if client in self.connected_clients:
            client.enqueue(Message.create_action_result(id, error).encode_cbor())
        if error is not None:
            logger.info(f"Rejected {describe(path, args)}: {error}")

    async def start(
        self,
        host: str = "127.0.0.1",
        port: int = 3000,
        path: str = "/ws/outpost",
        recorder: Optional[RecordingWriter] = None,
    ) -> None:
        """Starts the WebSocket server, recording to `recorder` if given, which gets the
        units and the actions sent before first."""
        self.path = path.rstrip("/")
        self._server = await serve(self._handle_connection, host, port)
        self.recorder = recorder
        if recorder:
            for frame in self.cached_frames():
                recorder.write(frame)

    async def stop(self) -> None:
        """Closes the WebSocket server and its connections, cancels the invocations
        under way and finishes the recording."""
        if self._server:
            # also closes the connections and waits for their handlers
            self._server.close()
            await self._server.wait_closed()
            self._server = None
        for client in self.connected_clients:
            client.close()
        self.connected_clients.clear()

        for task in list(self._invocations):
            task.cancel()
        if self.recorder:
            self.recorder.close()
            self.recorder = None
