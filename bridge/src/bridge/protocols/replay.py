"""Replays a recording file to blip at the pace of its timestamps."""

import asyncio
import logging
import time
from pathlib import Path
from typing import Optional, Union

import cbor2

from bridge.protocols.base import BaseProtocol
from bridge.recording import read_frames

logger = logging.getLogger("bridge.replay")


def timestamp(frame: bytes) -> Optional[int]:
    """The timestamp of the message in `frame` in nanoseconds, if it has one."""
    try:
        message = cbor2.loads(frame)
    except cbor2.CBORDecodeError:
        return None
    if not isinstance(message, dict) or len(message) != 1:
        return None
    content = next(iter(message.values()))
    value = content.get("timestamp") if isinstance(content, dict) else None
    return value if isinstance(value, int) else None


class ReplayProtocol(BaseProtocol):
    """Sends the messages of a recording file unchanged, each after the time between
    its timestamp and the first one divided by `speed`. Messages without a timestamp
    are sent right away. Starts when the first client connects, so it sees the whole
    recording."""

    def __init__(self, path: Union[str, Path], speed: float = 1.0) -> None:
        super().__init__()
        self.path = Path(path)
        self.speed = speed
        self.client_connected = asyncio.Event()
        self._task: Optional[asyncio.Task] = None

    async def start(self) -> None:
        # read now, so a missing or wrong file stops the bridge at startup
        self._frames = list(read_frames(self.path))
        logger.info(f"Replaying {len(self._frames)} messages of {self.path} at {self.speed}x")
        self._task = asyncio.create_task(self._replay())

    async def stop(self) -> None:
        if self._task:
            self._task.cancel()
            try:
                await self._task
            except asyncio.CancelledError:
                pass

    async def on_client_connected(self) -> None:
        self.client_connected.set()

    async def _replay(self) -> None:
        await self.client_connected.wait()
        first: Optional[int] = None
        start = time.monotonic()
        for frame in self._frames:
            stamp = timestamp(frame)
            if stamp is not None:
                if first is None:
                    first = stamp
                due = start + (stamp - first) / 1e9 / self.speed
                delay = due - time.monotonic()
                if delay > 0:
                    await asyncio.sleep(delay)
            await self.send_to_websocket(frame)
        logger.info("Replay finished")

    async def handle_incoming_ws_message(self, message: Union[bytes, str]) -> None:
        logger.debug("Ignoring a message from blip during a replay")
