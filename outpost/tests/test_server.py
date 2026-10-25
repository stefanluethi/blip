"""Tests for sending to blip clients that do not keep up."""

import asyncio
import logging
import struct
from typing import List

import websockets

from blip_outpost.server import OutpostServer

# large enough to fill the socket buffers of a client that does not read, uncompressed
MESSAGE_SIZE = 128 * 1024
MESSAGES = 400


async def no_actions(path: str, args: object) -> None:
    raise AssertionError("no invocations expected")


def message(index: int) -> bytes:
    # not a CBOR map, so not taken for actions or their results
    return b"\x00" + struct.pack(">I", index) + bytes(MESSAGE_SIZE)


def index(message: bytes) -> int:
    return struct.unpack(">I", message[1:5])[0]


async def serve(port: int, max_queued: int) -> OutpostServer:
    server = OutpostServer(no_actions, max_queued=max_queued)
    await server.start("127.0.0.1", port, "/ws/test")
    return server


def connect(port: int, **kwargs):
    return websockets.connect(f"ws://127.0.0.1:{port}/ws/test", compression=None, max_size=None, **kwargs)


async def receive_until(ws, last: int) -> List[int]:
    """The indices of the messages received up to the one of `last`."""
    received = []
    while not received or received[-1] != last:
        received.append(index(await asyncio.wait_for(ws.recv(), timeout=5.0)))
    return received


async def broadcast_all(server: OutpostServer) -> None:
    for i in range(MESSAGES):
        server.broadcast(message(i))
        # lets the senders run, as a protocol receiving from a target would
        await asyncio.sleep(0)


async def test_a_client_that_does_not_read_does_not_block_the_others():
    server = await serve(3979, max_queued=MESSAGES)
    try:
        async with connect(3979, max_queue=1) as stalled, connect(3979) as reading:
            await asyncio.sleep(0.05)
            reader = asyncio.create_task(receive_until(reading, MESSAGES - 1))
            await asyncio.wait_for(broadcast_all(server), timeout=10.0)
            assert await reader == list(range(MESSAGES))

            # the messages for the stalled client wait in its queue, none were dropped
            (client,) = [c for c in server.connected_clients if c.websocket.remote_address[1] == stalled.local_address[1]]
            assert client.queue.qsize() > 0
            assert client.dropped == 0
            # and arrive in order once it reads
            assert await receive_until(stalled, MESSAGES - 1) == list(range(MESSAGES))
    finally:
        await server.stop()


async def test_messages_go_together_unless_a_client_asks_for_one_per_frame():
    from blip_outpost.proto import Message

    server = await serve(3977, max_queued=MESSAGES)
    messages = [Message.create_value("a", index, index).encode_cbor() for index in range(50)]
    try:
        async with connect(3977) as batched, websockets.connect(
            "ws://127.0.0.1:3977/ws/test?batch=0"
        ) as single:
            await asyncio.sleep(0.05)
            # queued at once, before the senders run
            for message in messages:
                server.broadcast(message)
            frames = [await asyncio.wait_for(single.recv(), timeout=2.0) for _ in messages]
            assert frames == messages
            joined = b""
            while len(joined) < len(b"".join(messages)):
                joined += await asyncio.wait_for(batched.recv(), timeout=2.0)
            assert joined == b"".join(messages)
    finally:
        await server.stop()


async def test_messages_for_a_client_that_does_not_keep_up_are_dropped(caplog):
    caplog.set_level(logging.WARNING, logger="blip_outpost.server")
    server = await serve(3978, max_queued=4)
    try:
        async with connect(3978, max_queue=1) as stalled:
            await asyncio.sleep(0.05)
            await asyncio.wait_for(broadcast_all(server), timeout=10.0)
            (client,) = server.connected_clients
            assert client.dropped > 0
            full = [r for r in caplog.records if "does not keep up" in r.getMessage()]
            assert len(full) == 1

            # once it reads again, it gets the messages before and after the gap in order
            reader = asyncio.create_task(receive_until(stalled, MESSAGES))
            while client.queue.full():
                await asyncio.sleep(0.01)
            server.broadcast(message(MESSAGES))
            received = await reader
            assert received == sorted(received)
            assert len(received) < MESSAGES
            assert any("Dropped" in r.getMessage() for r in caplog.records)
    finally:
        await server.stop()
