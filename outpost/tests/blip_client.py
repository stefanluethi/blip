"""Receiving as blip does: a frame holds one message or several as a CBOR sequence."""

import asyncio
from typing import List

import cbor2
import websockets

from blip_outpost import Outpost
from blip_outpost.proto import split_items


def connect(port: int):
    """Connects to the outpost served at `port` on its default path."""
    return websockets.connect(f"ws://127.0.0.1:{port}/ws/outpost")


def sent(outpost: Outpost) -> List[dict]:
    """Collects what `outpost` passes on while it does not serve, decoded."""
    messages: List[dict] = []
    outpost._server.broadcast = lambda frame: messages.append(cbor2.loads(frame))  # type: ignore[method-assign]
    return messages


async def receive(ws, timeout: float = 2.0) -> bytes:
    """The next message from the outpost at `ws`, from the frame received last or
    else from the next one."""
    pending = ws.__dict__.setdefault("pending_messages", [])
    while not pending:
        frame = await asyncio.wait_for(ws.recv(), timeout=timeout)
        messages, rest = split_items(frame)
        assert rest == b"", "a frame holds whole messages"
        pending.extend(messages)
    return pending.pop(0)


async def messages_until(ws, variant: str, count: int = 1) -> List[dict]:
    """The messages received until `count` of `variant` arrived."""
    received: List[dict] = []
    while sum(variant in message for message in received) < count:
        received.append(cbor2.loads(await receive(ws, 2.0)))
    return received
