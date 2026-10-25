"""Tests of the fast protocol, the stress test of blip, and of waiting for room at the
clients."""

import asyncio
from collections import defaultdict

import cbor2
import pytest
import websockets

from outpost.loading import LoadError, create_protocol, load_protocol
from outpost.proto import Message
from outpost.protocols.fast import (
    DRAIN_EVERY,
    FastProtocol,
    due,
    encode_float_value,
    encode_stamp,
    signal_frequencies,
    value_middle,
)
from outpost.server import OutpostServer, _Client

from blip_client import receive


def capture(protocol: FastProtocol) -> list:
    """The messages `protocol` sends, decoded."""
    sent: list = []

    async def send(frame: bytes) -> None:
        sent.append(cbor2.loads(frame))

    protocol.attach(send)
    return sent


def test_the_template_encodes_a_value_as_cbor2_does():
    stamp = 1_761_480_000_123_456_789
    for value in (0.0, -1.5, 0.1, 1e300):
        expected = Message.create_value("fast/signal/07", value, stamp).encode_cbor()
        assert encode_float_value(encode_stamp(stamp), value_middle("fast/signal/07"), value) == expected
    # which cbor2 shortens to a half float
    encoded = encode_float_value(encode_stamp(stamp), value_middle("fast/signal/07"), float("inf"))
    assert cbor2.loads(encoded) == Message.create_value("fast/signal/07", float("inf"), stamp).to_dict()


def test_due_counts_the_samples_before_a_time():
    assert due(0.0, 100.0) == 0
    assert due(0.005, 100.0) == 1
    assert due(1.0, 100.0) == 100
    assert due(1.0, 0.0) == 0


def test_signal_frequencies_span_what_the_rate_shows():
    frequencies = signal_frequencies(5, 1000.0)
    assert frequencies[0] == pytest.approx(0.05)
    assert frequencies[-1] == pytest.approx(50.0)
    assert frequencies == sorted(frequencies)
    assert signal_frequencies(1, 1000.0) == [1.0]
    assert signal_frequencies(0, 1000.0) == []


async def test_it_sends_what_is_due_once():
    protocol = FastProtocol(
        traces=3, rate_hz=100, array_size=8, frame_size=4, array_rate_hz=10, log_rate_hz=50
    )
    sent = capture(protocol)
    protocol._origin_ns = 1_000_000_000_000

    await protocol.send_until(1.0)
    values = [m["Value"] for m in sent if "Value" in m]
    logs = [m["Log"] for m in sent if "Log" in m]
    by_path = defaultdict(list)
    for value in values:
        by_path[value["path"]].append(value)
    assert sorted(by_path) == [
        "fast/frame", "fast/signal/0", "fast/signal/1", "fast/signal/2", "fast/spectrum",
    ]
    for path in ("fast/signal/0", "fast/signal/1", "fast/signal/2"):
        stamps = [v["timestamp"] for v in by_path[path]]
        assert stamps == [1_000_000_000_000 + k * 10_000_000 for k in range(100)]
        assert all(isinstance(v["value"], float) for v in by_path[path])
    assert len(by_path["fast/spectrum"]) == 10
    assert all(len(v["value"]) == 8 for v in by_path["fast/spectrum"])
    assert len(by_path["fast/frame"]) == 10
    assert all(len(v["value"]) == 4 and all(len(row) == 4 for row in v["value"])
               for v in by_path["fast/frame"])
    assert len(logs) == 50
    assert all(set(log) == {"timestamp", "level", "component", "message"} for log in logs)
    assert [log["timestamp"] for log in logs] == sorted(log["timestamp"] for log in logs)
    assert protocol.messages == len(sent) == 300 + 10 + 10 + 50

    # nothing twice, and the rest later
    await protocol.send_until(1.0)
    assert len(sent) == 370
    await protocol.send_until(1.5)
    assert len(sent) == 370 + 150 + 5 + 5 + 25


async def test_it_sends_only_what_is_asked_for():
    protocol = FastProtocol(traces=0, array_size=0, frame_size=0, log_rate_hz=0)
    sent = capture(protocol)
    await protocol.send_until(1.0)
    assert sent == []
    assert protocol.messages_per_second() == 0


def test_options_are_checked():
    factory = load_protocol("fast")
    assert factory is FastProtocol
    assert create_protocol(factory, ["traces=5", "history_s=2.5"]).traces == 5
    with pytest.raises(LoadError, match="rate_hz must not be negative"):
        create_protocol(factory, ["rate_hz=-1"])


class GatedSocket:
    """A connection whose sends wait until the gate is opened."""

    remote_address = ("test", 0)

    def __init__(self) -> None:
        self.frames: list = []
        self.gate = asyncio.Event()

    async def send(self, frame: bytes) -> None:
        await self.gate.wait()
        self.frames.append(frame)


async def test_only_history_waits_for_the_start_action():
    assert "fast/start" not in [d["path"] for d in FastProtocol().actions.descriptors()]
    protocol = FastProtocol(history_s=1.0)
    assert [d["path"] for d in protocol.actions.descriptors()] == ["fast/start"]
    await protocol.on_client_connected()
    assert not protocol._started.is_set()
    await protocol.invoke("fast/start", {})
    assert protocol._started.is_set()
    assert not protocol.actions.is_enabled("fast/start")


async def test_drain_waits_until_the_queue_is_half_empty():
    client = _Client(GatedSocket(), max_queued=10)  # type: ignore[arg-type]
    for _ in range(6):
        client.enqueue(b"\x01")
    drain = asyncio.create_task(client.drain())
    await asyncio.sleep(0.01)
    assert not drain.done()

    # takes all queued messages in one frame, then waits at the gate
    sender = asyncio.create_task(client.send_queued())
    await asyncio.wait_for(drain, 1.0)
    sender.cancel()


async def test_drain_ends_when_the_client_is_gone():
    client = _Client(GatedSocket(), max_queued=10)  # type: ignore[arg-type]
    for _ in range(10):
        client.enqueue(b"\x01")
    drain = asyncio.create_task(client.drain())
    await asyncio.sleep(0.01)
    assert not drain.done()
    client.close()
    await asyncio.wait_for(drain, 1.0)


async def test_drain_returns_without_clients():
    server = OutpostServer(protocol=FastProtocol(), port=3989)
    await asyncio.wait_for(server.protocol.drain(), 1.0)


async def test_the_history_reaches_a_client_without_losing_messages():
    traces, rate_hz, history_s = 10, 1000.0, 2.0
    protocol = FastProtocol(
        traces=traces, rate_hz=rate_hz, array_size=0, frame_size=0, log_rate_hz=0,
        history_s=history_s,
    )
    # a queue that holds one more batch than it may be at, after waiting for room
    server = OutpostServer(
        protocol=protocol, host="127.0.0.1", port=3988, path="/ws/test", max_queued=2 * DRAIN_EVERY + 1
    )
    await server.start()
    try:
        async with websockets.connect("ws://127.0.0.1:3988/ws/test") as ws:
            await asyncio.sleep(0.2)
            assert protocol.messages == 0, "the history waits for fast/start"
            await protocol.invoke("fast/start", {})
            expected = int(history_s * rate_hz)
            stamps = defaultdict(list)
            while min((len(s) for s in stamps.values()), default=0) < expected or len(stamps) < traces:
                message = cbor2.loads(await receive(ws, 5.0))
                value = message.get("Value")
                if value and value["path"].startswith("fast/signal/"):
                    stamps[value["path"]].append(value["timestamp"])
            assert len(stamps) == traces
            for path_stamps in stamps.values():
                # one sample every millisecond, none lost
                steps = {b - a for a, b in zip(path_stamps[:expected], path_stamps[1:expected])}
                assert steps == {1_000_000}
            assert not any(client.dropped for client in server.connected_clients)
    finally:
        await server.stop()
