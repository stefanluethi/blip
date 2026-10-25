"""Receiving as blip does: a frame holds one message or several as a CBOR sequence."""

import asyncio

from blip_outpost.proto import split_items


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
