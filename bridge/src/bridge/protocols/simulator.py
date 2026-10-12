"""Simulator protocol implementation matching Rust simulator app."""

import asyncio
import random
import logging
from typing import Union
import psutil

from bridge.protocols.base import BaseProtocol
from bridge.proto import Message

logger = logging.getLogger("bridge.simulator")


class SimulatorProtocol(BaseProtocol):
    """Simulator protocol driving synthetic telemetry (CPU usage & accelerometer)."""

    def __init__(self, interval_ms: int = 50) -> None:
        super().__init__()
        self.interval_sec = interval_ms / 1000.0
        self._task: asyncio.Task | None = None
        self._running = False

    async def start(self) -> None:
        """Start simulator telemetry stream task."""
        self._running = True
        self._task = asyncio.create_task(self._run_loop())
        logger.info(f"Simulator started (sampling interval: {self.interval_sec * 1000:.0f}ms)")

    async def stop(self) -> None:
        """Stop simulator background task."""
        self._running = False
        if self._task:
            self._task.cancel()
            try:
                await self._task
            except asyncio.CancelledError:
                pass
        logger.info("Simulator stopped.")

    async def _run_loop(self) -> None:
        """Periodic loop broadcasting accelerometer and CPU telemetry encoded in CBOR."""
        # Initialize psutil cpu percent calculation
        psutil.cpu_percent(percpu=True)
        
        while self._running:
            try:
                # 1. Accelerometer simulation
                accel_val = random.uniform(-10.0, 10.0)
                accel_msg = Message.create_value("accelerometer", accel_val)
                await self.send_to_websocket(accel_msg.encode_cbor())

                # 2. Per-CPU usage simulation
                cpu_percentages = psutil.cpu_percent(percpu=True)
                for idx, cpu_val in enumerate(cpu_percentages):
                    cpu_path = f"sysinfo/cpu/cpu{idx}"
                    cpu_msg = Message.create_value(cpu_path, float(cpu_val))
                    await self.send_to_websocket(cpu_msg.encode_cbor())

                await asyncio.sleep(self.interval_sec)
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in simulator loop: {e}")
                await asyncio.sleep(0.1)

    async def handle_incoming_ws_message(self, message: Union[bytes, str]) -> None:
        """Echo incoming messages back to WebSocket clients (matching simulator behavior)."""
        logger.debug(f"Simulator received WS message, echoing back (size: {len(message)})")
        await self.send_to_websocket(message)
