"""Passes through the messages of a target that speaks the blip protocol itself, over
TCP or UDP, e.g. firmware using sonde."""

import asyncio
import logging
from typing import Optional

import cbor2

from blip_outpost.actions import ActionError
from blip_outpost.outpost import Outpost
from blip_outpost.proto import split_items

logger = logging.getLogger(__name__)

# Seconds before connecting again after an error
RETRY_DELAY_SEC = 3.0
# Bytes of an incomplete message kept at most, so a damaged length does not make the
# outpost wait for data forever
MAX_PENDING_BYTES = 16 * 1024 * 1024


class SocketPassthrough:
    """Passes the messages a target sends over TCP or UDP to the blips of `outpost`
    unchanged, and forwards their invocations to the target as Invoke requests. The
    target announces its actions itself, an invocation succeeds once it is forwarded.
    The ActionResult messages of the target are dropped by the outpost, which answered
    already.

    The target sends its messages as a CBOR sequence (RFC 8742), any number at once;
    over TCP they may be split anywhere, a UDP datagram holds complete messages. With
    `listen`, the passthrough waits for the target at `host` and `port` instead of
    connecting to it."""

    def __init__(
        self,
        outpost: Outpost,
        mode: str = "tcp",
        host: str = "127.0.0.1",
        port: int = 9000,
        listen: bool = False,
    ) -> None:
        self.mode = mode.lower()
        if self.mode not in ("tcp", "udp"):
            raise ValueError(f"unknown mode '{mode}', expected tcp or udp")
        self.outpost = outpost
        self.host = host
        self.port = port
        self.listen = listen

        self.writer: Optional[asyncio.StreamWriter] = None
        self._udp_transport: Optional[asyncio.DatagramTransport] = None
        self._next_id = 0
        outpost.on_invoke = self.invoke

    async def run(self) -> None:
        """Passes the messages through until cancelled, connecting again after errors."""
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
            logger.info("Socket passthrough stopped.")

    async def _tcp_client_loop(self) -> None:
        """Connect as TCP client and forward data."""
        while True:
            try:
                logger.info(f"Connecting to TCP server {self.host}:{self.port}...")
                reader, self.writer = await asyncio.open_connection(self.host, self.port)
                logger.info(f"Connected to TCP server {self.host}:{self.port}")
                await self._forward(reader)
                logger.warning("TCP server closed connection.")
            # any error, so the outpost keeps serving blip; cancelling is no Exception
            except Exception as e:
                logger.error(f"TCP client error: {e}. Reconnecting in {RETRY_DELAY_SEC:.0f} seconds...")
            self.writer = None
            await asyncio.sleep(RETRY_DELAY_SEC)

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
        """Sends each complete message of the TCP stream to blip, keeping the start of
        an incomplete one until the rest arrives."""
        pending = b""
        while True:
            data = await reader.read(4096)
            if not data:
                if pending:
                    logger.warning(f"Dropping {len(pending)} bytes of an incomplete message at the end of the stream")
                return
            pending += data
            try:
                frames, pending = split_items(pending)
            except ValueError as e:
                # the next message may start with a later read
                logger.error(f"Dropping {len(pending)} bytes that are no messages ({e}): {pending[:32].hex()}")
                pending = b""
                continue
            if len(pending) > MAX_PENDING_BYTES:
                logger.error(f"Dropping {len(pending)} bytes of a message that does not end: {pending[:32].hex()}")
                pending = b""
            for frame in frames:
                self.outpost._send_frame(frame)

    async def _udp(self) -> None:
        """Sets up the UDP endpoint, retrying while that fails."""
        outpost = self.outpost

        class UDPProtocol(asyncio.DatagramProtocol):
            def datagram_received(self, data: bytes, addr: tuple[str, int]) -> None:
                outpost.send(data)

        loop = asyncio.get_running_loop()
        while True:
            try:
                if self.listen:
                    transport, _ = await loop.create_datagram_endpoint(
                        UDPProtocol, local_addr=(self.host, self.port)
                    )
                    logger.info(f"UDP listener active on {self.host}:{self.port}")
                else:
                    transport, _ = await loop.create_datagram_endpoint(
                        UDPProtocol, remote_addr=(self.host, self.port)
                    )
                    logger.info(f"UDP socket bound to remote {self.host}:{self.port}")
                break
            except Exception as e:
                logger.error(f"UDP error: {e}. Retrying in {RETRY_DELAY_SEC:.0f} seconds...")
                await asyncio.sleep(RETRY_DELAY_SEC)
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
