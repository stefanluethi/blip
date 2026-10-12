"""Socket (TCP/UDP) protocol implementation."""

import asyncio
import logging
from typing import Union, Optional
from bridge.protocols.base import BaseProtocol

logger = logging.getLogger("bridge.socket")


class SocketProtocol(BaseProtocol):
    """Bridge for TCP and UDP sockets to WebSockets."""

    def __init__(
        self,
        mode: str = "tcp",
        remote_host: str = "127.0.0.1",
        remote_port: int = 9000,
        listen: bool = False
    ) -> None:
        super().__init__()
        self.mode = mode.lower()
        self.host = remote_host
        self.port = remote_port
        self.listen = listen

        self.reader: Optional[asyncio.StreamReader] = None
        self.writer: Optional[asyncio.StreamWriter] = None
        self.server: Optional[asyncio.Server] = None
        self._udp_transport: Optional[asyncio.DatagramTransport] = None

        self._task: Optional[asyncio.Task] = None
        self._running = False

    async def start(self) -> None:
        """Start TCP/UDP socket worker task."""
        self._running = True
        if self.mode == "tcp":
            if self.listen:
                self.server = await asyncio.start_server(self._handle_tcp_client, self.host, self.port)
                logger.info(f"TCP server listening on {self.host}:{self.port}")
            else:
                self._task = asyncio.create_task(self._tcp_client_loop())
        elif self.mode == "udp":
            self._task = asyncio.create_task(self._udp_loop())

    async def stop(self) -> None:
        """Stop socket operations and cleanup."""
        self._running = False
        if self._task:
            self._task.cancel()
            try:
                await self._task
            except asyncio.CancelledError:
                pass

        if self.server:
            self.server.close()
            await self.server.wait_closed()

        if self.writer:
            self.writer.close()
            try:
                await self.writer.wait_closed()
            except Exception:
                pass

        if self._udp_transport:
            self._udp_transport.close()

        logger.info("Socket protocol stopped.")

    async def _tcp_client_loop(self) -> None:
        """Connect as TCP client and forward data."""
        while self._running:
            try:
                logger.info(f"Connecting to TCP server {self.host}:{self.port}...")
                self.reader, self.writer = await asyncio.open_connection(self.host, self.port)
                logger.info(f"Connected to TCP server {self.host}:{self.port}")
                
                while self._running:
                    data = await self.reader.read(4096)
                    if not data:
                        logger.warning("TCP server closed connection.")
                        break
                    await self.send_to_websocket(data)

            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"TCP client error: {e}. Reconnecting in 3 seconds...")
                await asyncio.sleep(3.0)

    async def _handle_tcp_client(self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
        """Handle incoming TCP client connection when running in listen mode."""
        self.reader = reader
        self.writer = writer
        peer = writer.get_extra_info('peername')
        logger.info(f"Accepted TCP connection from {peer}")

        try:
            while self._running:
                data = await reader.read(4096)
                if not data:
                    break
                await self.send_to_websocket(data)
        except Exception as e:
            logger.error(f"Error handling TCP client {peer}: {e}")
        finally:
            writer.close()
            await writer.wait_closed()

    async def _udp_loop(self) -> None:
        """Setup UDP endpoint."""
        class UDPProtocol(asyncio.DatagramProtocol):
            def __init__(self, outer: 'SocketProtocol') -> None:
                self.outer = outer

            def datagram_received(self, data: bytes, addr: tuple[str, int]) -> None:
                asyncio.create_task(self.outer.send_to_websocket(data))

        loop = asyncio.get_running_loop()
        try:
            if self.listen:
                transport, _ = await loop.create_datagram_endpoint(
                    lambda: UDPProtocol(self), local_addr=(self.host, self.port)
                )
                logger.info(f"UDP listener active on {self.host}:{self.port}")
            else:
                transport, _ = await loop.create_datagram_endpoint(
                    lambda: UDPProtocol(self), remote_addr=(self.host, self.port)
                )
                logger.info(f"UDP socket bound to remote {self.host}:{self.port}")

            self._udp_transport = transport
            while self._running:
                await asyncio.sleep(1.0)
        except asyncio.CancelledError:
            pass
        except Exception as e:
            logger.error(f"UDP error: {e}")

    async def handle_incoming_ws_message(self, message: Union[bytes, str]) -> None:
        """Forward message from WS client to TCP/UDP socket."""
        payload = message.encode("utf-8") if isinstance(message, str) else message

        if self.mode == "tcp" and self.writer:
            try:
                self.writer.write(payload)
                await self.writer.drain()
            except Exception as e:
                logger.error(f"Error sending data to TCP socket: {e}")
        elif self.mode == "udp" and self._udp_transport:
            try:
                self._udp_transport.sendto(payload)
            except Exception as e:
                logger.error(f"Error sending data to UDP socket: {e}")
