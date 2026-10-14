"""Async WebSocket Server hosting protocol bridge connection."""

import asyncio
import logging
from typing import Set, Union, Optional
import websockets
from websockets.server import WebSocketServerProtocol, serve

from bridge.protocols.base import BaseProtocol

logger = logging.getLogger("bridge.server")


class BridgeServer:
    """WebSocket server bridging protocol drivers to WS clients."""

    def __init__(
        self,
        protocol: BaseProtocol,
        host: str = "127.0.0.1",
        port: int = 3000,
        path: str = "/ws/bridge"
    ) -> None:
        self.protocol = protocol
        self.host = host
        self.port = port
        self.path = path.rstrip("/")

        self.connected_clients: Set[WebSocketServerProtocol] = set()
        self._server: Optional[websockets.WebSocketServer] = None

        # Register callback with protocol backend
        self.protocol.register_broadcast_callback(self.broadcast)

    async def broadcast(self, message: Union[bytes, str]) -> None:
        """Broadcast message received from protocol backend to all active WS clients."""
        if not self.connected_clients:
            return

        # Prepare send tasks for active clients
        to_remove = set()
        # a copy, as clients may disconnect while a message is sent
        for client in list(self.connected_clients):
            try:
                await client.send(message)
            except websockets.ConnectionClosed:
                to_remove.add(client)
            except Exception as e:
                logger.error(f"Error broadcasting to client {client.remote_address}: {e}")
                to_remove.add(client)

        if to_remove:
            self.connected_clients.difference_update(to_remove)

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
        self.connected_clients.add(websocket)

        try:
            async for message in websocket:
                logger.debug(f"Received WS message from {client_addr} ({type(message).__name__}, len={len(message)})")
                await self.protocol.handle_incoming_ws_message(message)
        except websockets.ConnectionClosedOK:
            logger.info(f"WebSocket client {client_addr} disconnected cleanly")
        except websockets.ConnectionClosedError as e:
            logger.warning(f"WebSocket client {client_addr} connection closed with error: {e}")
        except Exception as e:
            logger.error(f"Error handling WebSocket client {client_addr}: {e}")
        finally:
            self.connected_clients.discard(websocket)

    async def start(self) -> None:
        """Start protocol and WebSocket server."""
        await self.protocol.start()
        
        logger.info(f"Starting WebSocket server on ws://{self.host}:{self.port}{self.path or '/'}")
        self._server = await serve(self._handle_connection, self.host, self.port)

    async def stop(self) -> None:
        """Close WebSocket server and stop protocol backend."""
        logger.info("Shutting down WebSocket server...")
        if self._server:
            self._server.close()
            await self._server.wait_closed()

        # Close all active connections
        if self.connected_clients:
            close_tasks = [client.close(reason="Server shutting down") for client in list(self.connected_clients)]
            await asyncio.gather(*close_tasks, return_exceptions=True)
            self.connected_clients.clear()

        await self.protocol.stop()
        logger.info("WebSocket server shutdown complete.")
