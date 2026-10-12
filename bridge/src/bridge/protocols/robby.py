"""Robby protocol: WebSocket client to the robby ESP32 communication module.

The ESP32 (robby ``app_comm``) forwards the COBS decoded CBOR messages of the
robby main controller as binary WebSocket frames on ``ws://<host>/ws/robby``,
and forwards binary frames it receives to the main controller.

Robby messages use their own schema (``robby/app_web/proto``)::

    {"payload": {"StreamDescriptor": {...}}}
    {"payload": {"StreamFrame": {...}}}
    {"payload": {"Log": {...}}}
    {"payload": {"ThreadTable": {...}}}
    {"payload": {"DistanceMap": {...}}}

They are translated to blip messages here. Actions sent by blip share the
encoding of robby's ``Action`` enum and are forwarded unchanged.
"""

import asyncio
import logging
import time
from dataclasses import dataclass
from typing import Any, Callable, Optional, Union

import cbor2
import websockets

from bridge.protocols.base import BaseProtocol
from bridge.proto import Message

logger = logging.getLogger("bridge.robby")

# Highest log level known to blip (error). Robby additionally knows fatal (5).
MAX_LOG_LEVEL = 4

# Component shown in blip for log records of the robby main controller, which
# does not transmit the log context.
LOG_COMPONENT = "robby"

# Reconstructed sample timestamps are re-anchored to the host clock when they
# deviate more than this from it (e.g. after a reconnect or a stalled link).
RESYNC_THRESHOLD_NS = 500_000_000

SEQUENCE_MODULO = 1 << 16

RECONNECT_DELAY_SEC = 3.0


def stream_path(name: str) -> str:
    """Map a robby stream name (``acc::x``) to a blip value path (``acc/x``)."""
    return name.replace("::", "/")


@dataclass
class _Stream:
    path: str
    unit: str
    scale_factor: float
    sampling_time_ns: float
    last_sequence: Optional[int] = None
    next_timestamp_ns: float = 0.0


class RobbyTranslator:
    """Translates robby CBOR messages to blip messages.

    Stream frames only carry raw samples and a sequence counter. Values are
    scaled with the stream descriptor's scale factor, and sample timestamps are
    reconstructed from the sampling time, anchored to the host clock.
    """

    def __init__(self, clock_ns: Callable[[], int] = time.time_ns) -> None:
        self._clock_ns = clock_ns
        self._streams: dict[int, _Stream] = {}
        self._ignored: set[str] = set()

    def translate(self, raw: bytes) -> list[Message]:
        """Translate one robby message, returning the resulting blip messages."""
        try:
            data = cbor2.loads(raw)
            payload = data["payload"]
            (variant, content), = payload.items()
        except Exception as e:
            logger.warning(f"Dropping undecodable robby message ({len(raw)}B): {e}")
            return []

        handler = {
            "StreamDescriptor": self._stream_descriptor,
            "StreamFrame": self._stream_frame,
            "Log": self._log,
            "ThreadTable": self._thread_table,
        }.get(variant)
        if handler is None:
            if variant not in self._ignored:
                self._ignored.add(variant)
                logger.info(f"Ignoring robby '{variant}' messages (not supported by blip)")
            return []

        try:
            return handler(content)
        except Exception as e:
            logger.warning(f"Dropping malformed robby '{variant}' message: {e}")
            return []

    def _stream_descriptor(self, descriptor: dict[str, Any]) -> list[Message]:
        stream_id = descriptor["id"]
        path = stream_path(descriptor["name"])
        unit = descriptor["unit"]
        scale_factor = float(descriptor["scale_factor"])
        sampling_time_ns = float(descriptor["sampling_time"]) * 1e9

        stream = self._streams.get(stream_id)
        if stream is not None and (stream.path, stream.unit) == (path, unit):
            # descriptors are repeated periodically, keep the timing state
            stream.scale_factor = scale_factor
            stream.sampling_time_ns = sampling_time_ns
        else:
            logger.info(f"Stream {stream_id}: '{path}' [{unit}]")
            self._streams[stream_id] = _Stream(path, unit, scale_factor, sampling_time_ns)

        # forwarded every time, so blip clients connecting later get it too
        return [Message.create_metadata(path, unit)]

    def _stream_frame(self, frame: dict[str, Any]) -> list[Message]:
        stream = self._streams.get(frame["id"])
        if stream is None:
            # without descriptor the samples can't be scaled or timed
            logger.debug(f"Dropping frame of stream {frame['id']} without descriptor")
            return []

        values = frame["values"]
        sequence = frame["sequence"]
        if not values:
            return []

        if stream.last_sequence is not None:
            missed = (sequence - stream.last_sequence - 1) % SEQUENCE_MODULO
            if missed:
                logger.warning(f"Stream '{stream.path}': {missed} frame(s) lost")
                stream.next_timestamp_ns += missed * len(values) * stream.sampling_time_ns
        stream.last_sequence = sequence

        # the newest sample was taken (at the latest) when the frame arrived
        anchored_ns = self._clock_ns() - (len(values) - 1) * stream.sampling_time_ns
        if abs(stream.next_timestamp_ns - anchored_ns) > RESYNC_THRESHOLD_NS:
            stream.next_timestamp_ns = anchored_ns

        messages = []
        for raw_value in values:
            messages.append(Message.create_value(
                stream.path,
                raw_value * stream.scale_factor,
                timestamp_ns=int(stream.next_timestamp_ns),
            ))
            stream.next_timestamp_ns += stream.sampling_time_ns
        return messages

    def _log(self, record: dict[str, Any]) -> list[Message]:
        # the controller has no time source for log records, use the host clock
        return [Message.create_log(
            min(record["level"], MAX_LOG_LEVEL),
            str(record["message"]),
            component=LOG_COMPONENT,
        )]

    def _thread_table(self, table: dict[str, Any]) -> list[Message]:
        messages = []
        timestamp_ns = self._clock_ns()
        for thread in table["threads"]:
            for field in ("runtime", "stack_usage"):
                path = f"threads/{thread['name']}/{field}"
                messages.append(Message.create_metadata(path, "%"))
                messages.append(Message.create_value(path, thread[field], timestamp_ns=timestamp_ns))
        return messages


class RobbyProtocol(BaseProtocol):
    """Bridge to the robby ESP32 WebSocket server."""

    def __init__(self, remote_host: str, remote_port: int = 80, remote_path: str = "/ws/robby") -> None:
        super().__init__()
        self.url = f"ws://{remote_host}:{remote_port}{remote_path}"
        self._ws: Optional[Any] = None
        self._task: Optional[asyncio.Task] = None
        self._running = False

    async def start(self) -> None:
        """Start the connection task."""
        self._running = True
        self._task = asyncio.create_task(self._client_loop())

    async def stop(self) -> None:
        """Close the connection and stop the connection task."""
        self._running = False
        if self._task:
            self._task.cancel()
            try:
                await self._task
            except asyncio.CancelledError:
                pass
        logger.info("Robby protocol stopped.")

    async def _client_loop(self) -> None:
        """Connect to robby, translate received messages and reconnect on errors."""
        while self._running:
            try:
                logger.info(f"Connecting to robby at {self.url}...")
                # robby does not answer pings, keepalive would drop the connection
                async with websockets.connect(self.url, ping_interval=None, compression=None) as ws:
                    self._ws = ws
                    logger.info(f"Connected to robby at {self.url}")
                    # timing and metadata start over with each session
                    translator = RobbyTranslator()
                    async for raw in ws:
                        if isinstance(raw, str):
                            continue
                        for message in translator.translate(raw):
                            await self.send_to_websocket(message.encode_cbor())
                    logger.warning("Robby closed the connection.")
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Robby connection error: {e}")
            finally:
                self._ws = None

            if self._running:
                logger.info(f"Reconnecting in {RECONNECT_DELAY_SEC:.0f} seconds...")
                await asyncio.sleep(RECONNECT_DELAY_SEC)

    async def handle_incoming_ws_message(self, message: Union[bytes, str]) -> None:
        """Forward actions from blip to robby."""
        if self._ws is None:
            logger.warning("Not connected to robby, dropping action")
            return
        payload = message.encode("utf-8") if isinstance(message, str) else message
        try:
            await self._ws.send(payload)
        except Exception as e:
            logger.error(f"Error sending action to robby: {e}")
