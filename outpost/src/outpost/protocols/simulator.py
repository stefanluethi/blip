"""Simulator protocol implementation matching Rust simulator app.

It only uses the public API of the outpost, so it is the reference for protocols of
users. Its actions change what it sends: the motor follows its speed setpoint, the log
level filters its log records and the ambient temperature shows in the thermal frame.

Its traces are sampled at different rates, like the sensors of a real target: the
accelerometer and the IMU every sampling interval, the others at their own periods,
which are no multiples of each other, so their samples rarely share a timestamp.
"""

import asyncio
import math
import random
import logging
import time
from typing import Annotated, Awaitable, Callable
import psutil

from outpost import ActionError, LogLevel, Message, Protocol, Range, Unit, action

logger = logging.getLogger("outpost.simulator")

TRACE, DEBUG, INFO, WARN, ERROR = LogLevel

# Smoothed per-core CPU load (percent) above which a warning is logged, and below
# which it is reported back to normal. Raw samples at short intervals jump between
# 0 and 100%, so the load is averaged over CPU_SMOOTHING_SEC first.
CPU_HIGH_THRESHOLD = 90.0
CPU_NORMAL_THRESHOLD = 70.0
CPU_SMOOTHING_SEC = 2.0

# Probability that the simulated temperature sensor fails to deliver a sample, which
# is sent as null
SENSOR_DROPOUT_PROBABILITY = 0.05

# The motor speed follows the setpoint with this time constant (s), and counts as
# running within this distance of it (rpm)
MOTOR_TIME_CONSTANT_SEC = 0.8
MOTOR_SETTLED_RPM = 30.0

# How long a calibration of the IMU takes (s)
CALIBRATION_SEC = 2.0

# The sampling periods (s) of the traces other than the accelerometer and the IMU,
# which are sampled every sampling interval
CPU_PERIOD_SEC = 0.25
MOTOR_PERIOD_SEC = 0.05
POSE_PERIOD_SEC = 0.1
SPECTRUM_PERIOD_SEC = 0.033
THERMAL_PERIOD_SEC = 0.115
TEMPERATURE_PERIOD_SEC = 1.0

# Units of the traces, sent as metadata every METADATA_INTERVAL_SEC so blip clients
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
POSE_LAP_SEC = 20.0

# A spectrum of this many bins with a peak sweeping across them in this time (s)
SPECTRUM_BINS = 64
SPECTRUM_SWEEP_SEC = 10.0

# A thermal camera frame of this many rows and columns with a hot spot circling in
# this time (s)
THERMAL_SIZE = 8
THERMAL_CIRCLE_SEC = 8.0

# Background log messages: (weight, level, component, message template)
_RANDOM_LOGS = [
    (30, TRACE, "sensor", "accelerometer sample {accel:.3f} m/s^2"),
    (20, DEBUG, "sensor", "accelerometer FIFO level {fifo}/32"),
    (15, DEBUG, "net", "heartbeat sent (seq={seq})"),
    (8, INFO, "storage", "flushed {blocks} blocks to flash"),
    (5, WARN, "net", "packet retransmit (seq={seq}, attempt {attempt})"),
    (4, WARN, "power", "supply voltage low: {voltage:.2f} V"),
    (2, ERROR, "sensor", "accelerometer I2C read timeout (addr=0x{addr:02x})"),
    (1, ERROR, "storage", "flash write failed at 0x{flash_addr:08x}, retrying"),
]


async def sleep_until(deadline: float) -> None:
    """Sleeps until `time.perf_counter()` reaches `deadline`. On Windows the event loop
    runs timers up to its clock resolution of about 16 ms early, so a single sleep of
    10 ms may return right away."""
    while (remaining := deadline - time.perf_counter()) > 0:
        await asyncio.sleep(remaining)


class SimulatorProtocol(Protocol):
    """Simulator protocol driving synthetic telemetry (CPU usage & accelerometer) and logs."""

    def __init__(self, interval_ms: int = 20, log_interval_ms: int = 500) -> None:
        self.interval_sec = interval_ms / 1000.0
        self.log_interval_sec = log_interval_ms / 1000.0
        self._seq = 0
        self._cpu_avg: dict[int, float] = {}
        self._cpu_high: set[int] = set()
        self._motor_enabled = True
        self._motor_setpoint = 1500
        self._motor_speed = 0.0
        self._log_level = TRACE
        self._ambient = 22.0
        self._gyro_bias = [random.gauss(0.0, 0.02) for _ in range(3)]
        self._accel = 0.0
        self._cpu_count = psutil.cpu_count() or 1
        # Start of the simulated time, which the slowly changing traces follow
        self._start = time.perf_counter()
        # set when a client connected, to send it the units right away
        self._client_connected = asyncio.Event()

    async def on_client_connected(self) -> None:
        self._client_connected.set()

    async def run(self) -> None:
        """Broadcasts the traces, each at its own rate, and logs encoded in CBOR."""
        logger.info(
            f"Simulator started (sampling interval: {self.interval_sec * 1000:.0f}ms, "
            f"log interval: {self.log_interval_sec * 1000:.0f}ms)"
        )
        # Initialize psutil cpu percent calculation
        self._cpu_count = len(psutil.cpu_percent(percpu=True))
        await self._send_log(
            INFO, "main", f"simulator booted ({self._cpu_count} CPU cores detected)"
        )
        self._start = time.perf_counter()
        await asyncio.gather(
            self._repeat_metadata(),
            self._every(self.interval_sec, self._send_accelerometer),
            self._every(self.interval_sec, lambda: self.send(self._imu_message())),
            self._every(CPU_PERIOD_SEC, self._send_cpu),
            self._every(MOTOR_PERIOD_SEC, lambda: self.send(self._motor_message())),
            self._every(POSE_PERIOD_SEC, lambda: self.send(self._pose_message())),
            self._every(SPECTRUM_PERIOD_SEC, lambda: self.send(self._spectrum_message())),
            self._every(THERMAL_PERIOD_SEC, lambda: self.send(self._thermal_message())),
            self._every(TEMPERATURE_PERIOD_SEC, self._send_temperature),
            self._send_random_logs(),
        )

    async def _every(self, period_sec: float, send: Callable[[], Awaitable[None]]) -> None:
        """Calls `send` every `period_sec` until cancelled, skipping the calls it is too
        late for."""
        next_tick = time.perf_counter()
        while True:
            try:
                await send()
            except Exception as e:
                logger.error(f"Error in simulator loop: {e}")
            next_tick = max(next_tick + period_sec, time.perf_counter())
            await sleep_until(next_tick)

    async def _repeat_metadata(self) -> None:
        """The units of the traces, repeated for clients connecting later and sent right
        away to a client that connected."""
        while True:
            self._client_connected.clear()
            try:
                await self._send_metadata()
            except Exception as e:
                logger.error(f"Error in simulator loop: {e}")
            try:
                await asyncio.wait_for(self._client_connected.wait(), METADATA_INTERVAL_SEC)
            except asyncio.TimeoutError:
                pass

    def _sim_time(self) -> float:
        """The simulated time (s) since the simulator started."""
        return time.perf_counter() - self._start

    async def _send_accelerometer(self) -> None:
        self._accel = random.uniform(-10.0, 10.0)
        await self.send_value("accelerometer", self._accel)

    async def _send_cpu(self) -> None:
        """The load (percent) of each core, and a log record when it changes range."""
        cpu_percentages = psutil.cpu_percent(percpu=True)
        for idx, cpu_val in enumerate(cpu_percentages):
            await self.send_value(f"sysinfo/cpu/cpu{idx}", float(cpu_val))
        await self._log_cpu_transitions(cpu_percentages)

    async def _send_temperature(self) -> None:
        """A sensor that sometimes delivers no value, sent as null."""
        temperature = None
        if random.random() >= SENSOR_DROPOUT_PROBABILITY:
            temperature = 25.0 + random.gauss(0.0, 0.2)
        await self.send_value("sensor/temperature", temperature)

    async def _send_random_logs(self) -> None:
        """Random background logs, at a jittered rate."""
        while True:
            await asyncio.sleep(self.log_interval_sec * random.uniform(0.5, 1.5))
            try:
                await self._send_random_log(self._accel)
            except Exception as e:
                logger.error(f"Error in simulator loop: {e}")

    def _imu_message(self) -> Message:
        """An IMU sample as a record of two vector arrays."""
        acc = [random.gauss(0.0, 0.1), random.gauss(0.0, 0.1), 9.81 + random.gauss(0.0, 0.1)]
        gyro = [bias + random.gauss(0.0, 0.01) for bias in self._gyro_bias]
        return Message.create_value("imu", {"acc": acc, "gyro": gyro})

    def _motor_message(self) -> Message:
        """A motor sample as a record of an integer, a bool and a string."""
        target = self._motor_setpoint if self._motor_enabled else 0.0
        alpha = min(1.0, MOTOR_PERIOD_SEC / MOTOR_TIME_CONSTANT_SEC)
        self._motor_speed += alpha * (target - self._motor_speed)
        if abs(target - self._motor_speed) > MOTOR_SETTLED_RPM:
            state = "accelerating" if target > self._motor_speed else "braking"
        else:
            state = "running" if target > 0 else "idle"
        speed_rpm = max(0, round(self._motor_speed + random.gauss(0.0, 5.0))) if state != "idle" else 0
        return Message.create_value(
            "motor", {"speed_rpm": speed_rpm, "enabled": self._motor_enabled, "state": state}
        )

    def _pose_message(self) -> Message:
        """The position (m) and heading (rad) of a robot driving a figure eight."""
        angle = 2.0 * math.pi * self._sim_time() / POSE_LAP_SEC
        x = POSE_EXTENT_M * math.sin(angle)
        y = POSE_EXTENT_M / 2.0 * math.sin(2.0 * angle)
        heading = math.atan2(math.cos(2.0 * angle), math.cos(angle))
        noise = lambda: random.gauss(0.0, 0.01)  # noqa: E731
        return Message.create_value(
            "robot/pose", {"x": x + noise(), "y": y + noise(), "heading": heading}
        )

    def _spectrum_message(self) -> Message:
        """A spectrum (dB) as an array of bins, with a peak sweeping back and forth."""
        phase = math.sin(2.0 * math.pi * self._sim_time() / SPECTRUM_SWEEP_SEC)
        peak = (SPECTRUM_BINS - 1) / 2.0 * (1.0 + 0.7 * phase)
        bins = [
            -60.0 + 50.0 * math.exp(-(((i - peak) / 3.0) ** 2)) + random.gauss(0.0, 2.0)
            for i in range(SPECTRUM_BINS)
        ]
        return Message.create_value("audio/spectrum", bins)

    def _thermal_message(self) -> Message:
        """A thermal camera frame (°C) as rows of pixels, with a hot spot circling."""
        angle = 2.0 * math.pi * self._sim_time() / THERMAL_CIRCLE_SEC
        center = (THERMAL_SIZE - 1) / 2.0
        spot_row = center + 2.5 * math.sin(angle)
        spot_column = center + 2.5 * math.cos(angle)
        frame = [
            [
                round(
                    self._ambient
                    + 13.0 * math.exp(-((row - spot_row) ** 2 + (column - spot_column) ** 2) / 2.0)
                    + random.gauss(0.0, 0.1),
                    2,
                )
                for column in range(THERMAL_SIZE)
            ]
            for row in range(THERMAL_SIZE)
        ]
        return Message.create_value("thermal/frame", frame)

    async def _send_metadata(self) -> None:
        """Broadcast the units of all traces."""
        units = dict(UNITS)
        units.update({f"sysinfo/cpu/cpu{idx}": CPU_UNIT for idx in range(self._cpu_count)})
        for path, unit in units.items():
            await self.send(Message.create_metadata(path, unit))
        await self._send_log_level()

    async def _send_log_level(self) -> None:
        """The log level as a trace, the readback of the log/level action."""
        await self.send_value("simulator/log_level", self._log_level.name)

    # The actions, offered to blip in this order

    @action("motor/enable", readback="motor.enabled")
    async def enable_motor(self, enabled: bool = True) -> None:
        """Switches the motor on or off"""
        self._motor_enabled = enabled
        await self._send_log(INFO, "motor", "motor enabled" if enabled else "motor disabled")

    @action("motor/speed", readback="motor.speed_rpm")
    async def set_motor_speed(self, rpm: Annotated[int, Range(0, 3000), Unit("rpm")] = 1500) -> None:
        """Speed setpoint of the motor"""
        if not self._motor_enabled:
            raise ActionError("the motor is disabled, enable it first")
        self._motor_setpoint = rpm
        await self._send_log(INFO, "motor", f"speed setpoint changed to {rpm} rpm")

    @action("thermal/ambient")
    async def set_ambient(
        self, temperature: Annotated[float, Range(15.0, 35.0, step=0.5), Unit("°C")] = 22.0
    ) -> None:
        """Ambient temperature of the thermal camera scene"""
        self._ambient = temperature

    @action("log/level", readback="simulator/log_level")
    async def set_log_level(self, level: LogLevel = LogLevel.TRACE) -> None:
        """Lowest level of the log records sent"""
        self._log_level = level
        await self._send_log_level()

    @action("log/message")
    async def log_message(self, level: LogLevel = LogLevel.INFO, text: str = "hello") -> None:
        """Sends a log record"""
        if not text.strip():
            raise ActionError("the text is empty")
        await self.send_log(level, text, component="user")

    @action("imu/calibrate", confirm=True)
    async def calibrate_imu(self) -> None:
        """Removes the bias of the gyroscope, takes a moment"""
        self.actions.set_enabled(False, "imu/calibrate")
        try:
            await self._send_log(INFO, "imu", "calibration started, keep the device still")
            await asyncio.sleep(CALIBRATION_SEC)
            self._gyro_bias = [0.0, 0.0, 0.0]
            await self._send_log(INFO, "imu", "calibration done")
        finally:
            self.actions.set_enabled(True, "imu/calibrate")

    async def _send_log(self, level: int, component: str, message: str) -> None:
        """Broadcast a single log record, unless below the log level."""
        if level < self._log_level:
            return
        log_msg = Message.create_log(level, message, component=component)
        await self.send(log_msg)

    async def _log_cpu_transitions(self, cpu_percentages: list[float]) -> None:
        """Log when a core's smoothed load enters or leaves the high-load range."""
        alpha = min(1.0, CPU_PERIOD_SEC / CPU_SMOOTHING_SEC)
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
            blocks=random.randint(1, 64),
            attempt=random.randint(1, 3),
            voltage=random.uniform(3.0, 3.2),
            addr=0x19,
            flash_addr=random.randrange(0x08000000, 0x08100000, 0x100),
        )
        await self._send_log(level, component, message)
