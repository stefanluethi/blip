"""Simulator protocol implementation matching Rust simulator app."""

import asyncio
import math
import random
import logging
import time
from typing import Union
import psutil

from bridge.protocols.base import BaseProtocol
from bridge.proto import Message

logger = logging.getLogger("bridge.simulator")

# Log levels as understood by blip (proto LogRecord.level)
TRACE, DEBUG, INFO, WARN, ERROR = range(5)

# Smoothed per-core CPU load (percent) above which a warning is logged, and below
# which it is reported back to normal. Raw samples at short intervals jump between
# 0 and 100%, so the load is averaged over CPU_SMOOTHING_SEC first.
CPU_HIGH_THRESHOLD = 90.0
CPU_NORMAL_THRESHOLD = 70.0
CPU_SMOOTHING_SEC = 2.0

# Probability that the simulated temperature sensor fails to deliver a sample, which
# is sent as null
SENSOR_DROPOUT_PROBABILITY = 0.05

MOTOR_STATES = ["idle", "accelerating", "running", "braking"]

# Units of the streams, sent as metadata every METADATA_INTERVAL_SEC so blip clients
# connecting later get them too. The fields of a record are joined to its path with "."
UNITS = {
    "accelerometer": "m/s²",
    "imu.acc": "m/s²",
    "imu.gyro": "rad/s",
    "motor.speed_rpm": "rpm",
    "robot/pose.x": "m",
    "robot/pose.y": "m",
    "robot/pose.heading": "rad",
    "audio/spectrum": "dB",
    "thermal/frame": "°C",
    "sensor/temperature": "°C",
}
CPU_UNIT = "%"
METADATA_INTERVAL_SEC = 1.0

# The simulated robot drives a figure eight of this size (m) in this time (s)
POSE_EXTENT_M = 2.0
POSE_PERIOD_SEC = 20.0

# A spectrum of this many bins with a peak sweeping across them in this time (s)
SPECTRUM_BINS = 64
SPECTRUM_SWEEP_SEC = 10.0

# A thermal camera frame of this many rows and columns with a hot spot circling in
# this time (s)
THERMAL_SIZE = 8
THERMAL_PERIOD_SEC = 8.0

# Background log messages: (weight, level, component, message template)
_RANDOM_LOGS = [
    (30, TRACE, "sensor", "accelerometer sample {accel:.3f} m/s^2"),
    (20, DEBUG, "sensor", "accelerometer FIFO level {fifo}/32"),
    (15, DEBUG, "net", "heartbeat sent (seq={seq})"),
    (10, INFO, "motor", "speed setpoint changed to {rpm} rpm"),
    (8, INFO, "storage", "flushed {blocks} blocks to flash"),
    (5, WARN, "net", "packet retransmit (seq={seq}, attempt {attempt})"),
    (4, WARN, "power", "supply voltage low: {voltage:.2f} V"),
    (2, ERROR, "sensor", "accelerometer I2C read timeout (addr=0x{addr:02x})"),
    (1, ERROR, "storage", "flash write failed at 0x{flash_addr:08x}, retrying"),
]


class SimulatorProtocol(BaseProtocol):
    """Simulator protocol driving synthetic telemetry (CPU usage & accelerometer) and logs."""

    def __init__(self, interval_ms: int = 50, log_interval_ms: int = 500) -> None:
        super().__init__()
        self.interval_sec = interval_ms / 1000.0
        self.log_interval_sec = log_interval_ms / 1000.0
        self._task: asyncio.Task | None = None
        self._running = False
        self._seq = 0
        self._cpu_avg: dict[int, float] = {}
        self._cpu_high: set[int] = set()
        self._motor_state = 0
        # Simulated time (s), advanced by the sampling interval
        self._sim_time = 0.0

    async def start(self) -> None:
        """Start simulator telemetry stream task."""
        self._running = True
        self._task = asyncio.create_task(self._run_loop())
        logger.info(
            f"Simulator started (sampling interval: {self.interval_sec * 1000:.0f}ms, "
            f"log interval: {self.log_interval_sec * 1000:.0f}ms)"
        )

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
        """Periodic loop broadcasting accelerometer and CPU telemetry and logs encoded in CBOR."""
        # Initialize psutil cpu percent calculation
        cpu_percentages = psutil.cpu_percent(percpu=True)
        await self._send_log(
            INFO, "main", f"simulator booted ({len(cpu_percentages)} CPU cores detected)"
        )
        next_log = time.monotonic() + self.log_interval_sec
        next_metadata = time.monotonic()

        while self._running:
            try:
                self._sim_time += self.interval_sec

                # 0. The units of the streams, repeated for clients connecting later
                if time.monotonic() >= next_metadata:
                    await self._send_metadata(len(cpu_percentages))
                    next_metadata = time.monotonic() + METADATA_INTERVAL_SEC

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
                await self._log_cpu_transitions(cpu_percentages)

                # 3. Typed values: a record with arrays, a record with an integer,
                # a bool and a string, and a sensor that sometimes delivers no value
                await self.send_to_websocket(self._imu_message().encode_cbor())
                await self.send_to_websocket(self._motor_message().encode_cbor())
                await self.send_to_websocket(self._pose_message().encode_cbor())
                await self.send_to_websocket(self._spectrum_message().encode_cbor())
                await self.send_to_websocket(self._thermal_message().encode_cbor())
                temperature = None
                if random.random() >= SENSOR_DROPOUT_PROBABILITY:
                    temperature = 25.0 + random.gauss(0.0, 0.2)
                temperature_msg = Message.create_value("sensor/temperature", temperature)
                await self.send_to_websocket(temperature_msg.encode_cbor())

                # 4. Random background logs, at a jittered rate
                now = time.monotonic()
                if now >= next_log:
                    await self._send_random_log(accel_val)
                    next_log = now + self.log_interval_sec * random.uniform(0.5, 1.5)

                await asyncio.sleep(self.interval_sec)
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in simulator loop: {e}")
                await asyncio.sleep(0.1)

    def _imu_message(self) -> Message:
        """An IMU sample as a record of two vector arrays."""
        acc = [random.gauss(0.0, 0.1), random.gauss(0.0, 0.1), 9.81 + random.gauss(0.0, 0.1)]
        gyro = [random.gauss(0.0, 0.01) for _ in range(3)]
        return Message.create_value("imu", {"acc": acc, "gyro": gyro})

    def _motor_message(self) -> Message:
        """A motor sample as a record of an integer, a bool and a string."""
        if random.random() < 0.02:
            self._motor_state = (self._motor_state + 1) % len(MOTOR_STATES)
        state = MOTOR_STATES[self._motor_state]
        enabled = state != "idle"
        speed_rpm = random.randrange(1400, 1600) if enabled else 0
        return Message.create_value(
            "motor", {"speed_rpm": speed_rpm, "enabled": enabled, "state": state}
        )

    def _pose_message(self) -> Message:
        """The position (m) and heading (rad) of a robot driving a figure eight."""
        angle = 2.0 * math.pi * self._sim_time / POSE_PERIOD_SEC
        x = POSE_EXTENT_M * math.sin(angle)
        y = POSE_EXTENT_M / 2.0 * math.sin(2.0 * angle)
        heading = math.atan2(math.cos(2.0 * angle), math.cos(angle))
        noise = lambda: random.gauss(0.0, 0.01)  # noqa: E731
        return Message.create_value(
            "robot/pose", {"x": x + noise(), "y": y + noise(), "heading": heading}
        )

    def _spectrum_message(self) -> Message:
        """A spectrum (dB) as an array of bins, with a peak sweeping back and forth."""
        phase = math.sin(2.0 * math.pi * self._sim_time / SPECTRUM_SWEEP_SEC)
        peak = (SPECTRUM_BINS - 1) / 2.0 * (1.0 + 0.7 * phase)
        bins = [
            -60.0 + 50.0 * math.exp(-(((i - peak) / 3.0) ** 2)) + random.gauss(0.0, 2.0)
            for i in range(SPECTRUM_BINS)
        ]
        return Message.create_value("audio/spectrum", bins)

    def _thermal_message(self) -> Message:
        """A thermal camera frame (°C) as rows of pixels, with a hot spot circling."""
        angle = 2.0 * math.pi * self._sim_time / THERMAL_PERIOD_SEC
        center = (THERMAL_SIZE - 1) / 2.0
        spot_row = center + 2.5 * math.sin(angle)
        spot_column = center + 2.5 * math.cos(angle)
        frame = [
            [
                round(
                    22.0
                    + 13.0 * math.exp(-((row - spot_row) ** 2 + (column - spot_column) ** 2) / 2.0)
                    + random.gauss(0.0, 0.1),
                    2,
                )
                for column in range(THERMAL_SIZE)
            ]
            for row in range(THERMAL_SIZE)
        ]
        return Message.create_value("thermal/frame", frame)

    async def _send_metadata(self, cpu_count: int) -> None:
        """Broadcast the units of all streams."""
        units = dict(UNITS)
        units.update({f"sysinfo/cpu/cpu{idx}": CPU_UNIT for idx in range(cpu_count)})
        for path, unit in units.items():
            await self.send_to_websocket(Message.create_metadata(path, unit).encode_cbor())

    async def _send_log(self, level: int, component: str, message: str) -> None:
        """Broadcast a single log record."""
        log_msg = Message.create_log(level, message, component=component)
        await self.send_to_websocket(log_msg.encode_cbor())

    async def _log_cpu_transitions(self, cpu_percentages: list[float]) -> None:
        """Log when a core's smoothed load enters or leaves the high-load range."""
        alpha = min(1.0, self.interval_sec / CPU_SMOOTHING_SEC)
        for idx, cpu_val in enumerate(cpu_percentages):
            avg = self._cpu_avg.get(idx, cpu_val)
            avg += alpha * (cpu_val - avg)
            self._cpu_avg[idx] = avg
            if avg >= CPU_HIGH_THRESHOLD and idx not in self._cpu_high:
                self._cpu_high.add(idx)
                await self._send_log(WARN, "sysinfo", f"cpu{idx} load high: {avg:.1f}%")
            elif avg < CPU_NORMAL_THRESHOLD and idx in self._cpu_high:
                self._cpu_high.discard(idx)
                await self._send_log(INFO, "sysinfo", f"cpu{idx} load back to normal: {avg:.1f}%")

    async def _send_random_log(self, accel_val: float) -> None:
        """Pick a weighted random background log message and broadcast it."""
        self._seq += 1
        _, level, component, template = random.choices(
            _RANDOM_LOGS, weights=[entry[0] for entry in _RANDOM_LOGS]
        )[0]
        message = template.format(
            accel=accel_val,
            fifo=random.randint(0, 32),
            seq=self._seq,
            rpm=random.randrange(0, 3001, 100),
            blocks=random.randint(1, 64),
            attempt=random.randint(1, 3),
            voltage=random.uniform(3.0, 3.2),
            addr=0x19,
            flash_addr=random.randrange(0x08000000, 0x08100000, 0x100),
        )
        await self._send_log(level, component, message)

    async def handle_incoming_ws_message(self, message: Union[bytes, str]) -> None:
        """Echo incoming messages back to WebSocket clients (matching simulator behavior)."""
        logger.debug(f"Simulator received WS message, echoing back (size: {len(message)})")
        await self.send_to_websocket(message)
