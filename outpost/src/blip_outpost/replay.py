"""Replays a recording file to blip at the pace of its timestamps."""

import asyncio
import logging
import time
from pathlib import Path
from typing import Optional

import cbor2

from blip_outpost.actions import ActionError
from blip_outpost.outpost import Outpost
from blip_outpost.proto import ACTIONS_PREFIX
from blip_outpost.recording import read_frames

logger = logging.getLogger(__name__)


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


def disable_actions(frame: bytes) -> bytes:
    """`frame` with its actions disabled if it is an Actions message, as a replay
    carries out no actions."""
    if not frame.startswith(ACTIONS_PREFIX):
        return frame
    try:
        message = cbor2.loads(frame)
        for descriptor in message["Actions"]:
            descriptor["enabled"] = False
        return cbor2.dumps(message)
    except Exception:
        return frame


class Replay:
    """Sends the messages of a recording file to the blips of `outpost` unchanged, each
    after the time between its timestamp and the first one divided by `speed`. Messages
    without a timestamp are sent right away. Starts when the first blip connects, so it
    sees the whole recording. The actions of the recording are shown disabled."""

    def __init__(self, outpost: Outpost, path: Path, speed: float = 1.0) -> None:
        self.outpost = outpost
        self.path = Path(path)
        self.speed = speed
        # read now, so a missing or wrong file stops the outpost at startup
        self._frames = list(read_frames(self.path))
        outpost.on_invoke = self.invoke
        logger.info(f"Replaying {len(self._frames)} messages of {self.path} at {self.speed}x")

    async def run(self) -> None:
        """Waits for a blip, then sends the recording; returns at its end."""
        await self.outpost.wait_for_client_async()
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
            self.outpost._send_frame(disable_actions(frame))
        logger.info("Replay finished")

    async def invoke(self, path: str, args: dict) -> None:
        raise ActionError("a replay carries out no actions")
