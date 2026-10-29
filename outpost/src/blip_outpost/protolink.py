"""Connects to a target that sends protobuf messages over a WebSocket and translates them
to blip, as the annotations of its schema say, see `blip_outpost.protobuf`.

On the WebSocket of the target:

- a binary frame from the target holds one serialized uplink message;
- a binary frame to the target holds one serialized downlink message (an action);
- a text frame from the target holds JSON values of its own, e.g. counters of a bridge
  that forwards the messages of a microcontroller:
  ``{"values": {"bridge/rx_bytes": 1234}, "units": {"bridge/rx_bytes": "B"}}``,
  optionally with a ``"timestamp"`` in ns; without one, they have the outpost's estimate
  of the time of the target.

A target that sends nothing for `idle_timeout` seconds counts as lost, and the outpost
connects again: a target that restarted without closing the connection would otherwise
never be noticed, as the outpost sends it nothing but actions.

The outpost publishes its own counters once per second: `outpost/frames`,
`outpost/decode_errors`, and with a sequence field `outpost/seq_gaps` and
`outpost/resets`.
"""

import asyncio
import json
import logging
from typing import Any, Optional

from websockets.asyncio.client import ClientConnection, connect
from websockets.exceptions import ConnectionClosed, ConnectionClosedOK

from blip_outpost.actions import ActionError
from blip_outpost.outpost import Outpost
from blip_outpost.protobuf import Invoker, Translator
from blip_outpost.schema import Schema

logger = logging.getLogger(__name__)

# Seconds before connecting again after an error
RETRY_DELAY_SEC = 3.0


class ProtobufLink:
    """Connects to the target at `url` (ws://...), connecting again whenever the
    connection is lost, and translates its messages of the type `uplink` for the blips of
    `outpost`. With `downlink`, the actions of that message type are offered to blip."""

    def __init__(
        self,
        outpost: Outpost,
        schema: Schema,
        url: str,
        uplink: str,
        downlink: Optional[str] = None,
        timeout: float = 1.0,
        stats_prefix: str = "outpost/",
        stats_period: float = 1.0,
        idle_timeout: Optional[float] = 5.0,
    ) -> None:
        self.outpost = outpost
        self.url = url
        # None or 0: wait for the target forever
        self.idle_timeout = idle_timeout or None
        self.stats_prefix = stats_prefix
        self.stats_period = stats_period
        self._ws: Optional[ClientConnection] = None
        self.invoker: Optional[Invoker] = None
        self.translator = Translator(schema, uplink, outpost, on_action_result=self._on_action_result)
        if downlink is not None:
            self.invoker = Invoker(
                schema,
                downlink,
                self.send,
                wait_for_results=self.translator.has_action_results,
                timeout=timeout,
            )
            self.invoker.install(outpost.actions)
            self._enable_actions(False)

    @property
    def connected(self) -> bool:
        return self._ws is not None

    async def run(self) -> None:
        """Translates until cancelled, connecting again after errors."""
        stats = asyncio.create_task(self._publish_stats())
        try:
            while True:
                try:
                    logger.info(f"Connecting to {self.url}...")
                    # the target need not answer pings, nor compress
                    # a lost target doesn't answer the close either
                    async with connect(
                        self.url, ping_interval=None, compression=None, open_timeout=10, close_timeout=1
                    ) as ws:
                        logger.info(f"Connected to {self.url}")
                        self._connected(ws)
                        try:
                            while True:
                                try:
                                    frame = await asyncio.wait_for(ws.recv(), self.idle_timeout)
                                except asyncio.TimeoutError:
                                    logger.error(f"Nothing from {self.url} for {self.idle_timeout:g} s, "
                                                 f"connecting again in {RETRY_DELAY_SEC:.0f} s")
                                    break
                                self._receive(frame)
                        except ConnectionClosedOK:
                            logger.warning("The target closed the connection")
                        finally:
                            self._disconnected()
                except (OSError, ConnectionClosed, asyncio.TimeoutError) as e:
                    logger.error(f"Connection to {self.url}: {e}. Connecting again in {RETRY_DELAY_SEC:.0f} s")
                except Exception as e:
                    # e.g. a refused upgrade, the outpost keeps serving blip
                    logger.error(f"Connection to {self.url}: {type(e).__name__}: {e}. "
                                 f"Connecting again in {RETRY_DELAY_SEC:.0f} s")
                await asyncio.sleep(RETRY_DELAY_SEC)
        finally:
            stats.cancel()

    def _connected(self, ws: ClientConnection) -> None:
        self._ws = ws
        self.translator.restart()
        self._enable_actions(True)

    def _disconnected(self) -> None:
        self._ws = None
        if self.invoker is not None:
            self.invoker.disconnected()
        self._enable_actions(False)

    def _enable_actions(self, enabled: bool) -> None:
        """The actions of the target can be invoked only while it is connected."""
        paths = [action.path for action in self.invoker.actions] if self.invoker is not None else []
        if paths:
            reason = None if enabled else "the target is not connected"
            self.outpost.actions.set_enabled(enabled, *paths, reason=reason)

    def _receive(self, frame: Any) -> None:
        if isinstance(frame, bytes):
            self.translator.feed(frame)
        else:
            self._json(frame)

    def _json(self, text: str) -> None:
        try:
            content = json.loads(text)
            if not isinstance(content, dict):
                raise ValueError("not an object")
            units = content.get("units", {})
            values = content.get("values", {})
            timestamp = content.get("timestamp")
            if not isinstance(units, dict) or not isinstance(values, dict):
                raise ValueError("units and values must be objects")
            if timestamp is not None and not isinstance(timestamp, int):
                raise ValueError("the timestamp must be an integer")
        except ValueError as e:
            logger.warning(f"Dropping a text frame that is no values ({e}): {text[:80]!r}")
            return
        for path, unit in units.items():
            self.outpost.send_metadata(str(path), str(unit))
        t = timestamp if timestamp is not None else self.translator.target_time_ns()
        for path, value in values.items():
            self.outpost.send_value(str(path), value, t)

    def _on_action_result(self, id: int, error: str) -> None:
        if self.invoker is not None:
            self.invoker.resolve(id, error)

    async def send(self, payload: bytes) -> None:
        """Sends a serialized downlink message, raises ActionError if not connected."""
        ws = self._ws
        if ws is None:
            raise ActionError("not connected to the target")
        try:
            await ws.send(payload)
        except ConnectionClosed:
            raise ActionError("the connection to the target was lost") from None

    async def _publish_stats(self) -> None:
        loop = asyncio.get_running_loop()
        next_at = loop.time()
        while True:
            next_at += self.stats_period
            await asyncio.sleep(max(0.0, next_at - loop.time()))
            t = self.translator.target_time_ns()
            for name, value in self.translator.counters.items():
                self.outpost.send_value(self.stats_prefix + name, value, t)
