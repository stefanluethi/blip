"""A stress test of blip: sends many traces at high rates, large arrays and frames and
a flood of log records, to see how fast blip takes in data, how it renders a lot of it
and how it copes with much data over time.

Data of the past (`history_s`) is sent first, as fast as the slowest blip takes it,
then the data of each moment as it is due. When blip falls behind, the protocol waits
for it instead of dropping messages, and reports how far behind real time it is. It
reports what it sends every second, in its log and as traces under ``fast/stats``.

Unlike the simulator it encodes its messages from templates, ten times as fast as
`Message`, and sends them with the internal `_send_frame`, so it measures blip rather
than the encoding in Python. Your own scripts use `send_value`.

    python examples/fast.py [--traces 100] [--rate-hz 1000] [--history-s 600] ...
"""

import argparse
import asyncio
import itertools
import logging
import math
import random
import struct
import time
from typing import List

import cbor2

from blip_outpost import LogLevel, Outpost, action, sleep_until

logger = logging.getLogger("fast")

# How often the data that is due is sent (s), at most this much of it (s) at once
TICK_SEC = 0.01
MAX_CHUNK_SEC = 0.05

# Waits for room at the blips after this many messages, less than half the queue of a
# blip, so none is dropped
DRAIN_EVERY = 1000

STATS_INTERVAL_SEC = 1.0

# The signal traces are sine, square, sawtooth and random walk shapes, in turn, at
# frequencies (Hz) from MIN_FREQUENCY_HZ up to the sample rate divided by
# SAMPLES_PER_PERIOD, plus noise of this amplitude
SHAPES = 4
MIN_FREQUENCY_HZ = 0.05
SAMPLES_PER_PERIOD = 20
NOISE = 0.05

# The peak of the spectrum sweeps across it in this time (s), the hot spot of the
# frame circles in this time (s)
SPECTRUM_SWEEP_SEC = 10.0
FRAME_CIRCLE_SEC = 8.0

# Background log records: (weight, level, component, message template). Every
# LONG_LOG_EVERY-th record is a long one
_LOGS = [
    (30, LogLevel.TRACE, "sensor", "sample {n} read in {us} us"),
    (25, LogLevel.DEBUG, "net", "frame {n} sent, {bytes} bytes"),
    (20, LogLevel.INFO, "storage", "block {n} written to flash"),
    (12, LogLevel.WARN, "net", "frame {n} retransmitted, attempt {attempt}"),
    (8, LogLevel.WARN, "power", "supply voltage low: {voltage:.2f} V"),
    (5, LogLevel.ERROR, "sensor", "read {n} timed out after {us} us"),
]
_LOG_WEIGHTS = list(itertools.accumulate(entry[0] for entry in _LOGS))
LONG_LOG_EVERY = 100
LONG_LOG_WORDS = 80

UNITS = {
    "fast/spectrum": "dB",
    "fast/frame": "°C",
    "fast/stats/messages_per_s": "1/s",
    "fast/stats/megabytes_per_s": "MB/s",
    "fast/stats/lag_s": "s",
}

# A Value message is its head, the timestamp, the middle with its path and the value
_VALUE_HEAD = b"\xa1" + cbor2.dumps("Value") + b"\xa3" + cbor2.dumps("timestamp")
_STAMP = struct.Struct(">BQ").pack  # a CBOR unsigned integer of 64 bits
_FLOAT = struct.Struct(">Bd").pack  # a CBOR float of 64 bits
_UINT64, _FLOAT64 = 0x1B, 0xFB


def value_middle(path: str) -> bytes:
    """The bytes of a Value message between its timestamp and its value."""
    return cbor2.dumps("path") + cbor2.dumps(path) + cbor2.dumps("value")


def encode_float_value(stamp: bytes, middle: bytes, value: float) -> bytes:
    """A Value message of a float, `stamp` from `encode_stamp`, as cbor2 encodes it."""
    return _VALUE_HEAD + stamp + middle + _FLOAT(_FLOAT64, value)


def encode_stamp(timestamp_ns: int) -> bytes:
    return _STAMP(_UINT64, timestamp_ns)


class _Signal:
    """A float trace of a shape and frequency."""

    def __init__(self, path: str, shape: int, frequency_hz: float) -> None:
        self.path = path
        self.middle = value_middle(path)
        self.shape = shape
        self.frequency_hz = frequency_hz
        self.phase = random.uniform(0.0, 2.0 * math.pi)
        self.walk = 0.0

    def value(self, t: float) -> float:
        noise = NOISE * (random.random() - 0.5)
        if self.shape == 0:
            return math.sin(2.0 * math.pi * self.frequency_hz * t + self.phase) + noise
        if self.shape == 1:
            up = math.sin(2.0 * math.pi * self.frequency_hz * t + self.phase) >= 0.0
            return (1.0 if up else -1.0) + noise
        if self.shape == 2:
            cycle = self.frequency_hz * t + self.phase / (2.0 * math.pi)
            return 2.0 * (cycle - math.floor(cycle)) - 1.0 + noise
        # a random walk drawn back to zero, which stays within about ±1
        self.walk = 0.999 * self.walk + 0.05 * random.gauss(0.0, 1.0)
        return self.walk


def signal_frequencies(count: int, rate_hz: float) -> List[float]:
    """`count` frequencies (Hz) spread evenly on a log scale over what `rate_hz` can show."""
    highest = max(MIN_FREQUENCY_HZ, rate_hz / SAMPLES_PER_PERIOD)
    if count == 1:
        return [min(1.0, highest)]
    ratio = highest / MIN_FREQUENCY_HZ
    return [MIN_FREQUENCY_HZ * ratio ** (i / (count - 1)) for i in range(count)]


def due(end_s: float, rate_hz: float) -> int:
    """The number of samples at `rate_hz` before the time `end_s` (s), the first at 0."""
    return math.ceil(end_s * rate_hz) if rate_hz > 0 else 0


class Fast:
    """Sends to `outpost` `traces` float traces of `rate_hz` samples per second each, an
    array of `array_size` and a frame of `frame_size` × `frame_size` floats
    `array_rate_hz` times per second and `log_rate_hz` log records per second, after
    `history_s` seconds of data of the past.

    Starts when the first blip connects. With history, it waits for the action
    `fast/start` instead: blip keeps only what it receives while recording, which it
    starts once connected."""

    def __init__(
        self,
        outpost: Outpost,
        traces: int = 100,
        rate_hz: float = 1000.0,
        array_size: int = 1024,
        frame_size: int = 64,
        array_rate_hz: float = 30.0,
        log_rate_hz: float = 1000.0,
        history_s: float = 0.0,
    ) -> None:
        for name, value in [
            ("traces", traces), ("rate_hz", rate_hz), ("array_size", array_size),
            ("frame_size", frame_size), ("array_rate_hz", array_rate_hz),
            ("log_rate_hz", log_rate_hz), ("history_s", history_s),
        ]:
            if value < 0:
                raise ValueError(f"{name} must not be negative")
        self.outpost = outpost
        self.traces = traces
        self.rate_hz = float(rate_hz)
        self.array_size = array_size
        self.frame_size = frame_size
        self.array_rate_hz = float(array_rate_hz)
        self.log_rate_hz = float(log_rate_hz)
        self.history_s = float(history_s)

        digits = len(str(max(traces - 1, 0)))
        self._signals = [
            _Signal(f"fast/signal/{i:0{digits}d}", i % SHAPES, frequency)
            for i, frequency in enumerate(signal_frequencies(traces, self.rate_hz))
        ]
        self._spectrum_middle = value_middle("fast/spectrum")
        self._frame_middle = value_middle("fast/frame")
        # the next sample of each kind
        self._next_signal = 0
        self._next_array = 0
        self._next_log = 0
        # the time (ns) of the first sample
        self._origin_ns = 0
        # what was sent, by `_emit`
        self.messages = 0
        self.bytes = 0
        self._drain_at = DRAIN_EVERY
        self._started = asyncio.Event()
        if self.history_s > 0.0:
            outpost.register(self)

    @action("fast/start")
    async def start(self) -> None:
        """Sends the history, then the data as it is due; record in blip first"""
        self._started.set()
        self.outpost.actions.set_enabled(False, "fast/start")

    async def wait_for_start(self) -> None:
        """Waits for a blip, or with history for the action fast/start."""
        if self.history_s > 0.0:
            await self._started.wait()
        else:
            await self.outpost.wait_for_client_async()

    async def run(self) -> None:
        logger.info(
            f"Fast mode: {self.traces} traces at {self.rate_hz:g} Hz, "
            f"array of {self.array_size} and frame of {self.frame_size}x{self.frame_size} "
            f"at {self.array_rate_hz:g} Hz, {self.log_rate_hz:g} log records/s, "
            f"{self.history_s:g} s of history; about {self.messages_per_second():,.0f} "
            "messages/s. "
            + ("Waiting for fast/start: connect blip, start recording, then run it"
               if self.history_s else "Waiting for a blip to connect")
        )
        self._send_units()
        await self.wait_for_start()
        # the clock (perf_counter) at the time of the first sample
        start = time.perf_counter() - self.history_s
        self._origin_ns = time.time_ns() - round(self.history_s * 1e9)
        sent_s = 0.0  # the data before this time (s since the first sample) is sent
        caught_up = self.history_s == 0.0
        stats_at, stats_messages, stats_bytes = time.perf_counter(), 0, 0
        while True:
            now = time.perf_counter()
            elapsed_s = now - start
            if elapsed_s - sent_s < TICK_SEC:
                if not caught_up:
                    caught_up = True
                    logger.info(
                        f"Sent the {self.history_s:g} s of history in "
                        f"{elapsed_s - self.history_s:.2f} s, {self.messages:,} messages"
                    )
                await sleep_until(start + sent_s + TICK_SEC)
                continue
            end_s = min(elapsed_s, sent_s + MAX_CHUNK_SEC)
            await self.send_until(end_s)
            sent_s = end_s
            # lets the clients send what is queued
            await asyncio.sleep(0)

            if now - stats_at >= STATS_INTERVAL_SEC:
                self._send_stats(
                    now - stats_at, self.messages - stats_messages,
                    self.bytes - stats_bytes, elapsed_s - sent_s,
                )
                stats_at, stats_messages, stats_bytes = now, self.messages, self.bytes

    def messages_per_second(self) -> float:
        """The messages per second sent in real time."""
        arrays = (self.array_size > 0) + (self.frame_size > 0)
        return self.traces * self.rate_hz + arrays * self.array_rate_hz + self.log_rate_hz

    async def send_until(self, end_s: float) -> None:
        """Sends the samples and log records before the time `end_s` (s since the first
        sample) that were not sent yet."""
        await self._send_signals(end_s)
        await self._send_arrays(end_s)
        await self._send_logs(end_s)

    def _stamp(self, t: float) -> int:
        return self._origin_ns + round(t * 1e9)

    async def _emit(self, frame: bytes) -> None:
        """Sends the bytes of one message, waiting for room at the blips now and then."""
        self.messages += 1
        self.bytes += len(frame)
        self.outpost._send_frame(frame)
        if self.messages >= self._drain_at:
            self._drain_at = self.messages + DRAIN_EVERY
            await self.outpost.drain_async()

    async def _send_signals(self, end_s: float) -> None:
        if not self._signals:
            return
        until = due(end_s, self.rate_hz)
        for k in range(self._next_signal, until):
            t = k / self.rate_hz
            stamp = encode_stamp(self._stamp(t))
            for signal in self._signals:
                await self._emit(encode_float_value(stamp, signal.middle, signal.value(t)))
        self._next_signal = max(self._next_signal, until)

    async def _send_arrays(self, end_s: float) -> None:
        if self.array_size == 0 and self.frame_size == 0:
            return
        until = due(end_s, self.array_rate_hz)
        for k in range(self._next_array, until):
            t = k / self.array_rate_hz
            stamp = encode_stamp(self._stamp(t))
            if self.array_size:
                values = cbor2.dumps(self._spectrum(t))
                await self._emit(_VALUE_HEAD + stamp + self._spectrum_middle + values)
            if self.frame_size:
                values = cbor2.dumps(self._frame(t))
                await self._emit(_VALUE_HEAD + stamp + self._frame_middle + values)
        self._next_array = max(self._next_array, until)

    def _spectrum(self, t: float) -> List[float]:
        """A spectrum (dB) with a peak sweeping back and forth."""
        size = self.array_size
        peak = (size - 1) / 2.0 * (1.0 + 0.7 * math.sin(2.0 * math.pi * t / SPECTRUM_SWEEP_SEC))
        width = max(1.0, size / 40.0)
        return [
            -60.0 + 50.0 * math.exp(-(((i - peak) / width) ** 2)) + 4.0 * (random.random() - 0.5)
            for i in range(size)
        ]

    def _frame(self, t: float) -> List[List[float]]:
        """A thermal frame (°C) as rows, with a hot spot circling."""
        size = self.frame_size
        angle = 2.0 * math.pi * t / FRAME_CIRCLE_SEC
        center = (size - 1) / 2.0
        radius = 0.3 * size
        sigma = max(1.0, size / 8.0)
        spot_row = center + radius * math.sin(angle)
        spot_column = center + radius * math.cos(angle)
        # a Gaussian spot is the product of one along the rows and one along the columns
        rows = [math.exp(-0.5 * ((r - spot_row) / sigma) ** 2) for r in range(size)]
        columns = [math.exp(-0.5 * ((c - spot_column) / sigma) ** 2) for c in range(size)]
        return [
            [22.0 + 13.0 * row * column + 0.2 * (random.random() - 0.5) for column in columns]
            for row in rows
        ]

    async def _send_logs(self, end_s: float) -> None:
        until = due(end_s, self.log_rate_hz)
        for k in range(self._next_log, until):
            _, level, component, template = random.choices(_LOGS, cum_weights=_LOG_WEIGHTS)[0]
            message = template.format(
                n=k,
                us=random.randint(10, 5000),
                bytes=random.randint(16, 1500),
                attempt=random.randint(1, 3),
                voltage=random.uniform(3.0, 3.2),
            )
            if k % LONG_LOG_EVERY == LONG_LOG_EVERY - 1:
                message += ": " + " ".join(
                    random.choice(("lorem", "ipsum", "dolor", "sit", "amet", "consectetur"))
                    for _ in range(LONG_LOG_WORDS)
                )
            record = {
                "timestamp": self._stamp(k / self.log_rate_hz),
                "level": int(level),
                "component": component,
                "message": message,
            }
            await self._emit(cbor2.dumps({"Log": record}))
        self._next_log = max(self._next_log, until)

    def _send_units(self) -> None:
        for path, unit in UNITS.items():
            self.outpost.send_metadata(path, unit)

    def _send_stats(self, interval_s: float, messages: int, size: int, lag_s: float) -> None:
        """What was sent in the last `interval_s`, and how far behind real time it is."""
        per_second = messages / interval_s
        megabytes = size / interval_s / 1e6
        logger.info(
            f"Sent {per_second:,.0f} messages/s, {megabytes:.2f} MB/s, {lag_s:.2f} s behind"
        )
        now = time.time_ns()
        self.outpost.send_value("fast/stats/messages_per_s", per_second, now)
        self.outpost.send_value("fast/stats/megabytes_per_s", megabytes, now)
        self.outpost.send_value("fast/stats/lag_s", lag_s, now)


async def main(args: argparse.Namespace) -> None:
    async with Outpost(host=args.host, port=args.port, record=args.record) as outpost:
        logger.info(f"Serving blip on {outpost.url}, Ctrl+C to stop")
        fast = Fast(
            outpost, args.traces, args.rate_hz, args.array_size, args.frame_size,
            args.array_rate_hz, args.log_rate_hz, args.history_s,
        )
        await fast.run()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--host", default="127.0.0.1", help="address to serve blip on")
    parser.add_argument("--port", type=int, default=3000, help="port to serve blip on")
    parser.add_argument("--record", help="record what is sent to this file")
    parser.add_argument("--traces", type=int, default=100, help="float traces fast/signal/N")
    parser.add_argument("--rate-hz", type=float, default=1000.0, help="samples per second of each trace")
    parser.add_argument("--array-size", type=int, default=1024, help="elements of fast/spectrum, 0 for none")
    parser.add_argument("--frame-size", type=int, default=64, help="rows and columns of fast/frame, 0 for none")
    parser.add_argument("--array-rate-hz", type=float, default=30.0, help="samples per second of the array and frame")
    parser.add_argument("--log-rate-hz", type=float, default=1000.0, help="log records per second")
    parser.add_argument("--history-s", type=float, default=0.0,
                        help="seconds of the past sent first, once the action fast/start is run")
    logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s] %(name)s: %(message)s")
    try:
        asyncio.run(main(parser.parse_args()))
    except KeyboardInterrupt:
        pass
