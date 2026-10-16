"""Socket (TCP/UDP) protocol for targets that speak the blip protocol themselves."""

import asyncio
import logging
from typing import Optional

import cbor2

from bridge import ActionError, Protocol

logger = logging.getLogger("bridge.socket")


class SocketProtocol(Protocol):
    """Passes the data a target sends over TCP or UDP to blip unchanged, and forwards
    the invocations of blip to the target as Invoke requests. The target announces its
    actions itself, an invocation succeeds once it is forwarded."""

    def __init__(
        self,
        mode: str = "tcp",
        remote_host: str = "127.0.0.1",
        remote_port: int = 9000,
        listen: bool = False
    ) -> None:
        self.mode = mode.lower()
        if self.mode not in ("tcp", "udp"):
            raise ValueError(f"unknown mode '{mode}', expected tcp or udp")
        self.host = remote_host
        self.port = remote_port
        self.listen = listen

        self.writer: Optional[asyncio.StreamWriter] = None
        self._udp_transport: Optional[asyncio.DatagramTransport] = None
        self._next_id = 0

    async def run(self) -> None:
        try:
            if self.mode == "udp":
                await self._udp()
            elif self.listen:
                server = await asyncio.start_server(self._handle_tcp_client, self.host, self.port)
                logger.info(f"TCP server listening on {self.host}:{self.port}")
                async with server:
                    await server.serve_forever()
            else:
                await self._tcp_client_loop()
        finally:
            if self.writer:
                self.writer.close()
            if self._udp_transport:
                self._udp_transport.close()
            logger.info("Socket protocol stopped.")

    async def _tcp_client_loop(self) -> None:
        """Connect as TCP client and forward data."""
        while True:
            try:
                logger.info(f"Connecting to TCP server {self.host}:{self.port}...")
                reader, self.writer = await asyncio.open_connection(self.host, self.port)
                logger.info(f"Connected to TCP server {self.host}:{self.port}")
                await self._forward(reader)
                logger.warning("TCP server closed connection.")
            except (OSError, asyncio.IncompleteReadError) as e:
                logger.error(f"TCP client error: {e}. Reconnecting in 3 seconds...")
            self.writer = None
            await asyncio.sleep(3.0)

    async def _handle_tcp_client(self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
        """Handle incoming TCP client connection when running in listen mode."""
        self.writer = writer
        peer = writer.get_extra_info('peername')
        logger.info(f"Accepted TCP connection from {peer}")
        try:
            await self._forward(reader)
        except Exception as e:
            logger.error(f"Error handling TCP client {peer}: {e}")
        finally:
            self.writer = None
            writer.close()
            await writer.wait_closed()

    async def _forward(self, reader: asyncio.StreamReader) -> None:
        while True:
            data = await reader.read(4096)
            if not data:
                return
            await self.send(data)

    async def _udp(self) -> None:
        """Setup UDP endpoint."""
        class UDPProtocol(asyncio.DatagramProtocol):
            def __init__(self, outer: 'SocketProtocol') -> None:
                self.outer = outer

            def datagram_received(self, data: bytes, addr: tuple[str, int]) -> None:
                asyncio.create_task(self.outer.send(data))

        loop = asyncio.get_running_loop()
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
        await asyncio.Event().wait()

    async def invoke(self, path: str, args: dict) -> None:
        """Forwards the invocation to the target as an Invoke request."""
        self._next_id = (self._next_id + 1) % (1 << 32)
        request = cbor2.dumps({"Invoke": {"id": self._next_id, "path": path, "args": args}})
        if self.mode == "tcp" and self.writer:
            self.writer.write(request)
            await self.writer.drain()
        elif self.mode == "udp" and self._udp_transport:
            self._udp_transport.sendto(request)
        else:
            raise ActionError("not connected to the target")
