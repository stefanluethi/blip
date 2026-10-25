"""WebSocket server connecting blip clients to a protocol."""

import asyncio
import logging
from typing import Any, Optional, Set
from urllib.parse import parse_qs, urlsplit

import websockets
from websockets.asyncio.server import Server, ServerConnection, serve

from outpost.actions import ActionError
from outpost.proto import ACTION_RESULT_PREFIX, ACTIONS_PREFIX, LogLevel, Message, decode_request
from outpost.protocol import Protocol
from outpost.recording import RecordingWriter

logger = logging.getLogger("outpost.server")

# The component of the log records about invocations
ACTIONS_COMPONENT = "actions"

# The messages queued for a client are sent together in one WebSocket frame, as a CBOR
# sequence, up to this many bytes, so that a busy target does not cost a frame per
# message, neither here nor in blip. A client that reads one message per frame asks
# for that with `?batch=0` after the path.
MAX_BATCH_BYTES = 64 * 1024


def describe(path: str, args: Any) -> str:
    """An invocation as shown in log records, e.g. ``motor/speed(rpm=1500)``."""
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
            # joined once, as adding to bytes copies all before each time
            frame = b"".join(batch)
            if self.dropped:
                logger.warning(f"Dropped {self.dropped} messages for client {self.websocket.remote_address}")
                self.dropped = 0
            await self.websocket.send(frame)


class OutpostServer:
    """WebSocket server sending the messages of a protocol to all blip clients and
    passing the invocations of each client to the protocol."""

    def __init__(
        self,
        protocol: Protocol,
        host: str = "127.0.0.1",
        port: int = 3000,
        path: str = "/ws/outpost",
        recorder: Optional[RecordingWriter] = None,
        max_queued: int = 10000,
    ) -> None:
        self.protocol = protocol
        # records the binary messages, also while no client is connected
        self.recorder = recorder
        self.host = host
        self.port = port
        self.path = path.rstrip("/")
        # the messages queued for a client at most, more are dropped
        self.max_queued = max_queued

        self.connected_clients: Set[_Client] = set()
        self._server: Optional[Server] = None
        self._protocol_task: Optional[asyncio.Task] = None
        self._invocations: Set[asyncio.Task] = set()
        # the latest list of actions sent, for clients connecting later, also when the
        # target announces its actions itself
        self._actions_frame: Optional[bytes] = None

        self.protocol.attach(self.broadcast)

    async def broadcast(self, message: bytes) -> None:
        """Queues a message of the protocol for all clients, but answers to invocations.
        A single one, `Protocol.send` splits the batches of a target."""
        if message.startswith(ACTION_RESULT_PREFIX):
            # e.g. of a target that knows the actions itself, or of a replay: the outpost
            # answers the invoking client itself, and these ids are not that client's
            logger.debug("Dropping an ActionResult of the protocol")
            return
        if message.startswith(ACTIONS_PREFIX):
            self._actions_frame = message
        if self.recorder:
            self.recorder.write(message)
        for client in self.connected_clients:
            client.enqueue(message)

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
        logger.info(f"WebSocket client connected from {client_addr} on path '{path}'")
        # added and given the latest actions without awaiting in between, so a change
        # announced later is queued after the list it replaces
        client = _Client(websocket, self.max_queued, batched)
        self.connected_clients.add(client)
        if self._actions_frame is not None:
            client.enqueue(self._actions_frame)
        sender = asyncio.create_task(client.send_queued())

        try:
            await self.protocol.on_client_connected()
            async for message in websocket:
                if isinstance(message, str):
                    logger.warning(f"Ignoring a text message from {client_addr}")
                    continue
                self._start_invocation(client, message)
        except websockets.ConnectionClosedOK:
            logger.info(f"WebSocket client {client_addr} disconnected cleanly")
        except websockets.ConnectionClosedError as e:
            logger.warning(f"WebSocket client {client_addr} connection closed with error: {e}")
        except Exception as e:
            logger.error(f"Error handling WebSocket client {client_addr}: {e}")
        finally:
            self.connected_clients.discard(client)
            sender.cancel()
            try:
                await sender
            except (asyncio.CancelledError, websockets.ConnectionClosed):
                pass
            except Exception as e:
                logger.error(f"Error sending to WebSocket client {client_addr}: {e}")

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
            await self.protocol.invoke(path, args)
        except ActionError as e:
            error = str(e) or "rejected"
        except Exception as e:
            logger.exception(f"Invoking '{path}' failed")
            error = f"{type(e).__name__}: {e}"

        # in the client's queue, so it stays in order with the broadcast messages
        if client in self.connected_clients:
            client.enqueue(Message.create_action_result(id, error).encode_cbor())
        # to all clients and the recording, so they see what was invoked
        if error is None:
            log = Message.create_log(LogLevel.DEBUG, f"{describe(path, args)}: ok", component=ACTIONS_COMPONENT)
        else:
            logger.info(f"Rejected {describe(path, args)}: {error}")
            log = Message.create_log(
                LogLevel.WARN, f"{describe(path, args)} rejected: {error}", component=ACTIONS_COMPONENT
            )
        await self.broadcast(log.encode_cbor())

    @property
    def protocol_task(self) -> Optional[asyncio.Task]:
        """The task running the protocol."""
        return self._protocol_task

    async def start(self) -> None:
        """Start the protocol and the WebSocket server."""
        if len(self.protocol.actions):
            await self.protocol.announce_actions()
        self._protocol_task = asyncio.create_task(self.protocol.run())
        logger.info(f"Starting WebSocket server on ws://{self.host}:{self.port}{self.path or '/'}")
        self._server = await serve(self._handle_connection, self.host, self.port)

    async def stop(self) -> None:
        """Close WebSocket server and its connections, and stop the protocol."""
        logger.info("Shutting down WebSocket server...")
        if self._server:
            # also closes the connections and waits for their handlers
            self._server.close()
            await self._server.wait_closed()
        self.connected_clients.clear()

        for task in [self._protocol_task, *self._invocations]:
            if task is not None and not task.done():
                task.cancel()
        if self._protocol_task is not None:
            try:
                await self._protocol_task
            except asyncio.CancelledError:
                pass
            except Exception:
                # reported by whoever watched the task
                pass
        self.protocol.attach(None)
        if self.recorder:
            self.recorder.close()
        logger.info("WebSocket server shutdown complete.")
