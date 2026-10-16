"""WebSocket server connecting blip clients to a protocol."""

import asyncio
import logging
from typing import Any, Optional, Set

import websockets
from websockets.server import WebSocketServerProtocol, serve

from bridge.actions import ActionError
from bridge.proto import LogLevel, Message, decode_request
from bridge.protocol import Protocol
from bridge.recording import RecordingWriter

logger = logging.getLogger("bridge.server")

# The start of the CBOR encoding of an Actions message, a map of one entry whose key
# is the text "Actions"
_ACTIONS_PREFIX = b"\xa1\x67Actions"

# The component of the log records about invocations
ACTIONS_COMPONENT = "actions"


def describe(path: str, args: Any) -> str:
    """An invocation as shown in log records, e.g. ``motor/speed(rpm=1500)``."""
    if isinstance(args, dict):
        arguments = ", ".join(f"{name}={value!r}" for name, value in args.items())
    else:
        arguments = repr(args)
    return f"{path}({arguments})"


class BridgeServer:
    """WebSocket server sending the messages of a protocol to all blip clients and
    passing the invocations of each client to the protocol."""

    def __init__(
        self,
        protocol: Protocol,
        host: str = "127.0.0.1",
        port: int = 3000,
        path: str = "/ws/bridge",
        recorder: Optional[RecordingWriter] = None,
    ) -> None:
        self.protocol = protocol
        # records the binary messages, also while no client is connected
        self.recorder = recorder
        self.host = host
        self.port = port
        self.path = path.rstrip("/")

        self.connected_clients: Set[WebSocketServerProtocol] = set()
        self._server: Optional[websockets.WebSocketServer] = None
        self._protocol_task: Optional[asyncio.Task] = None
        self._invocations: Set[asyncio.Task] = set()
        # the latest list of actions sent, for clients connecting later, also when the
        # target announces its actions itself
        self._actions_frame: Optional[bytes] = None

        self.protocol.attach(self.broadcast)

    async def broadcast(self, message: bytes) -> None:
        """Sends a message of the protocol to all clients."""
        if message.startswith(_ACTIONS_PREFIX):
            self._actions_frame = message
        if self.recorder:
            self.recorder.write(message)
        # a copy, as clients may disconnect while a message is sent
        for client in list(self.connected_clients):
            await self._send(client, message)

    async def _send(self, client: WebSocketServerProtocol, message: bytes) -> None:
        try:
            await client.send(message)
        except websockets.ConnectionClosed:
            self.connected_clients.discard(client)
        except Exception as e:
            logger.error(f"Error sending to client {client.remote_address}: {e}")
            self.connected_clients.discard(client)

    async def _handle_connection(self, websocket: WebSocketServerProtocol, path: str) -> None:
        """Handle individual WebSocket client connection."""
        req_path = path.rstrip("/")
        # Allow connecting to target path or wildcard root
        if self.path and req_path != self.path and req_path != "":
            logger.warning(f"Connection rejected: Path '{req_path}' does not match expected '{self.path}'")
            await websocket.close(code=1008, reason="Invalid path")
            return

        client_addr = websocket.remote_address
        logger.info(f"WebSocket client connected from {client_addr} on path '{path}'")
        # added before the actions are sent, so a change announced meanwhile reaches the
        # client too; if it may have arrived before the list it replaces, the latest
        # list is sent once more
        self.connected_clients.add(websocket)
        sent = self._actions_frame
        if sent is not None:
            await self._send(websocket, sent)
            if self._actions_frame is not sent:
                await self._send(websocket, self._actions_frame)
        await self.protocol.on_client_connected()

        try:
            async for message in websocket:
                if isinstance(message, str):
                    logger.warning(f"Ignoring a text message from {client_addr}")
                    continue
                self._start_invocation(websocket, message)
        except websockets.ConnectionClosedOK:
            logger.info(f"WebSocket client {client_addr} disconnected cleanly")
        except websockets.ConnectionClosedError as e:
            logger.warning(f"WebSocket client {client_addr} connection closed with error: {e}")
        except Exception as e:
            logger.error(f"Error handling WebSocket client {client_addr}: {e}")
        finally:
            self.connected_clients.discard(websocket)

    def _start_invocation(self, client: WebSocketServerProtocol, frame: bytes) -> None:
        try:
            invocation = decode_request(frame)
        except ValueError as e:
            logger.warning(f"Ignoring a message from {client.remote_address}: {e}")
            return
        # in a task, so a slow action does not hold up the invocations of other actions
        task = asyncio.create_task(self._invoke(client, invocation.id, invocation.path, invocation.args))
        self._invocations.add(task)
        task.add_done_callback(self._invocations.discard)

    async def _invoke(self, client: WebSocketServerProtocol, id: int, path: str, args: Any) -> None:
        error: Optional[str] = None
        try:
            await self.protocol.invoke(path, args)
        except ActionError as e:
            error = str(e) or "rejected"
        except Exception as e:
            logger.exception(f"Invoking '{path}' failed")
            error = f"{type(e).__name__}: {e}"

        await self._send(client, Message.create_action_result(id, error).encode_cbor())
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
        """Close WebSocket server and stop the protocol."""
        logger.info("Shutting down WebSocket server...")
        if self._server:
            self._server.close()
            await self._server.wait_closed()

        # Close all active connections
        if self.connected_clients:
            close_tasks = [client.close(reason="Server shutting down") for client in list(self.connected_clients)]
            await asyncio.gather(*close_tasks, return_exceptions=True)
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
