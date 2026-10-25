"""A made-up target to try blip without hardware, served by an outpost.

    python examples/simulator.py [--port 3000] [--record simulator.blip]

It is also the reference for scripts with asyncio: `Simulator` stands for the code that
talks to a target, sends what it reads to the outpost and offers its actions as
methods. Its actions change what it sends: the motor follows its speed setpoint, the
log level filters its log records and the ambient temperature shows in the thermal
frame.

Its traces are sampled at different rates, like the sensors of a real target: the
accelerometer and the IMU every sampling interval, the others at their own periods,
which are no multiples of each other, so their samples rarely share a timestamp.
"""

import argparse
import asyncio
import logging
import math
import random
import time
from typing import Annotated, Callable

import psutil

from blip_outpost import ActionError, LogLevel, Message, Outpost, Range, Unit, action, sleep_until

logger = logging.getLogger("simulator")

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

# Units of the traces, sent once at the start: the outpost passes them on to blips
# connecting later. The fields of a record are joined to its path with "."
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


class Simulator:
    """Sends synthetic telemetry and log records to `outpost`, and offers its actions."""

    def __init__(self, outpost: Outpost, interval_ms: int = 20, log_interval_ms: int = 500) -> None:
        self.outpost = outpost
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
        # the methods decorated with @action become the actions of the outpost
        outpost.register(self)

    async def run(self) -> None:
        """Sends the traces, each at its own rate, and log records until cancelled."""
        logger.info(
            f"Simulator started (sampling interval: {self.interval_sec * 1000:.0f}ms, "
            f"log interval: {self.log_interval_sec * 1000:.0f}ms)"
        )
        # Initialize psutil cpu percent calculation
        self._cpu_count = len(psutil.cpu_percent(percpu=True))
        self._send_units()
        self._send_log(INFO, "main", f"simulator booted ({self._cpu_count} CPU cores detected)")
        self._start = time.perf_counter()
        send = self.outpost.send
        await asyncio.gather(
            self._every(self.interval_sec, self._send_accelerometer),
            self._every(self.interval_sec, lambda: send(self._imu_message())),
            self._every(CPU_PERIOD_SEC, self._send_cpu),
            self._every(MOTOR_PERIOD_SEC, lambda: send(self._motor_message())),
            self._every(POSE_PERIOD_SEC, lambda: send(self._pose_message())),
            self._every(SPECTRUM_PERIOD_SEC, lambda: send(self._spectrum_message())),
            self._every(THERMAL_PERIOD_SEC, lambda: send(self._thermal_message())),
            self._every(TEMPERATURE_PERIOD_SEC, self._send_temperature),
            self._send_random_logs(),
        )

    async def _every(self, period_sec: float, send: Callable[[], None]) -> None:
        """Calls `send` every `period_sec` until cancelled, skipping the calls it is too
        late for."""
        next_tick = time.perf_counter()
        while True:
            try:
                send()
            except Exception as e:
                logger.error(f"Error in simulator loop: {e}")
            next_tick = max(next_tick + period_sec, time.perf_counter())
            await sleep_until(next_tick)

    def _sim_time(self) -> float:
        """The simulated time (s) since the simulator started."""
        return time.perf_counter() - self._start

    def _send_units(self) -> None:
        """The units of all traces, and the log level."""
        units = dict(UNITS)
        units.update({f"sysinfo/cpu/cpu{idx}": CPU_UNIT for idx in range(self._cpu_count)})
        for path, unit in units.items():
            self.outpost.send_metadata(path, unit)
        self._send_log_level()

    def _send_accelerometer(self) -> None:
        self._accel = random.uniform(-10.0, 10.0)
        self.outpost.send_value("accelerometer", self._accel)

    def _send_cpu(self) -> None:
        """The load (percent) of each core, and a log record when it changes range."""
        cpu_percentages = psutil.cpu_percent(percpu=True)
        for idx, cpu_val in enumerate(cpu_percentages):
            self.outpost.send_value(f"sysinfo/cpu/cpu{idx}", float(cpu_val))
        self._log_cpu_transitions(cpu_percentages)

    def _send_temperature(self) -> None:
        """A sensor that sometimes delivers no value, sent as null."""
        temperature = None
        if random.random() >= SENSOR_DROPOUT_PROBABILITY:
            temperature = 25.0 + random.gauss(0.0, 0.2)
        self.outpost.send_value("sensor/temperature", temperature)

    async def _send_random_logs(self) -> None:
        """Random background logs, at a jittered rate."""
        while True:
            await asyncio.sleep(self.log_interval_sec * random.uniform(0.5, 1.5))
            try:
                self._send_random_log(self._accel)
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

    def _send_log_level(self) -> None:
        """The log level as a trace, the readback of the log/level action."""
        self.outpost.send_value("simulator/log_level", self._log_level.name)

    # The actions, offered to blip in this order

    @action("motor/enable", readback="motor.enabled")
    async def enable_motor(self, enabled: bool = True) -> None:
        """Switches the motor on or off"""
        self._motor_enabled = enabled
        self._send_log(INFO, "motor", "motor enabled" if enabled else "motor disabled")

    @action("motor/speed", readback="motor.speed_rpm")
    async def set_motor_speed(self, rpm: Annotated[int, Range(0, 3000), Unit("rpm")] = 1500) -> None:
        """Speed setpoint of the motor"""
        if not self._motor_enabled:
            raise ActionError("the motor is disabled, enable it first")
        self._motor_setpoint = rpm
        self._send_log(INFO, "motor", f"speed setpoint changed to {rpm} rpm")

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
        self._send_log_level()

    @action("log/message")
    async def log_message(self, level: LogLevel = LogLevel.INFO, text: str = "hello") -> None:
        """Sends a log record"""
        if not text.strip():
            raise ActionError("the text is empty")
        self.outpost.send_log(level, text, component="user")

    @action("imu/calibrate", confirm=True)
    async def calibrate_imu(self) -> None:
        """Removes the bias of the gyroscope, takes a moment"""
        self.outpost.actions.set_enabled(False, "imu/calibrate")
        try:
            self._send_log(INFO, "imu", "calibration started, keep the device still")
            await asyncio.sleep(CALIBRATION_SEC)
            self._gyro_bias = [0.0, 0.0, 0.0]
            self._send_log(INFO, "imu", "calibration done")
        finally:
            self.outpost.actions.set_enabled(True, "imu/calibrate")

    def _send_log(self, level: int, component: str, message: str) -> None:
        """Sends a single log record, unless below the log level."""
        if level < self._log_level:
            return
        self.outpost.send_log(level, message, component=component)

    def _log_cpu_transitions(self, cpu_percentages: list[float]) -> None:
        """Log when a core's smoothed load enters or leaves the high-load range."""
        alpha = min(1.0, CPU_PERIOD_SEC / CPU_SMOOTHING_SEC)
        for idx, cpu_val in enumerate(cpu_percentages):
            avg = self._cpu_avg.get(idx, cpu_val)
            avg += alpha * (cpu_val - avg)
            self._cpu_avg[idx] = avg
            if avg >= CPU_HIGH_THRESHOLD and idx not in self._cpu_high:
                self._cpu_high.add(idx)
                self._send_log(WARN, "sysinfo", f"cpu{idx} load high: {avg:.1f}%")
            elif avg < CPU_NORMAL_THRESHOLD and idx in self._cpu_high:
                self._cpu_high.discard(idx)
                self._send_log(INFO, "sysinfo", f"cpu{idx} load back to normal: {avg:.1f}%")

    def _send_random_log(self, accel_val: float) -> None:
        """Pick a weighted random background log message and send it."""
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
        self._send_log(level, component, message)


async def main(args: argparse.Namespace) -> None:
    async with Outpost(host=args.host, port=args.port, record=args.record) as outpost:
        logger.info(f"Serving blip on {outpost.url}, Ctrl+C to stop")
        await Simulator(outpost, args.interval_ms, args.log_interval_ms).run()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--host", default="127.0.0.1", help="address to serve blip on")
    parser.add_argument("--port", type=int, default=3000, help="port to serve blip on")
    parser.add_argument("--record", help="record what is sent to this file")
    parser.add_argument("--interval-ms", type=int, default=20, help="sampling interval")
    parser.add_argument("--log-interval-ms", type=int, default=500, help="mean time between log records")
    logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s] %(name)s: %(message)s")
    try:
        asyncio.run(main(parser.parse_args()))
    except KeyboardInterrupt:
        pass
