"""Tests for the robby protocol translation and bridge."""

import asyncio
import cbor2
import pytest
import websockets
from websockets.asyncio.server import serve

from bridge.server import BridgeServer
from bridge.protocols.robby import RobbyProtocol, RobbyTranslator
from bridge.proto import decode_cbor_message

SAMPLING_TIME = 0.001  # 1 ms


def robby_message(variant: str, content: dict) -> bytes:
    return cbor2.dumps({"payload": {variant: content}})


def descriptor(stream_id: int = 0, name: str = "acc::x") -> bytes:
    return robby_message("StreamDescriptor", {
        "id": stream_id,
        "name": name,
        "scale_factor": 0.5,
        "unit": "g",
        "sampling_time": SAMPLING_TIME,
    })


def frame(sequence: int, values: list[int], stream_id: int = 0) -> bytes:
    return robby_message("StreamFrame", {"id": stream_id, "sequence": sequence, "values": values})


class FakeClock:
    def __init__(self, now_ns: int = 1_000_000_000_000) -> None:
        self.now_ns = now_ns

    def __call__(self) -> int:
        return self.now_ns


def test_descriptor_emits_metadata():
    translator = RobbyTranslator()
    messages = translator.translate(descriptor())
    assert [m.to_dict() for m in messages] == [{"ValueMetadata": {"path": "acc/x", "unit": "g"}}]
    # repeated descriptors are forwarded for late joining clients
    assert len(translator.translate(descriptor())) == 1


def test_frame_without_descriptor_is_dropped():
    assert RobbyTranslator().translate(frame(0, [1, 2, 3])) == []


def test_frame_values_are_scaled_and_timed():
    clock = FakeClock()
    translator = RobbyTranslator(clock_ns=clock)
    translator.translate(descriptor())

    messages = translator.translate(frame(0, [2, 4, 6]))
    values = [m.data for m in messages]
    assert [v.value for v in values] == [1.0, 2.0, 3.0]
    assert all(v.path == "acc/x" for v in values)
    # newest sample at arrival, older ones one sampling time apart
    assert [v.timestamp for v in values] == [clock.now_ns - 2_000_000, clock.now_ns - 1_000_000, clock.now_ns]

    # the next frame continues seamlessly, independent of arrival jitter
    clock.now_ns += 5_000_000
    messages = translator.translate(frame(1, [8, 10, 12]))
    assert [m.data.timestamp for m in messages] == [values[-1].timestamp + i * 1_000_000 for i in (1, 2, 3)]


def test_lost_frames_leave_a_gap():
    clock = FakeClock()
    translator = RobbyTranslator(clock_ns=clock)
    translator.translate(descriptor())
    last = translator.translate(frame(65535, [0, 0]))[-1].data.timestamp

    # sequence wraps around, frames 0 and 1 are lost
    messages = translator.translate(frame(2, [0, 0]))
    assert messages[0].data.timestamp == last + 5 * 1_000_000


def test_timestamps_resync_to_host_clock():
    clock = FakeClock()
    translator = RobbyTranslator(clock_ns=clock)
    translator.translate(descriptor())
    translator.translate(frame(0, [0, 0]))

    clock.now_ns += 10_000_000_000  # link stalled for 10 s
    messages = translator.translate(frame(1, [0, 0]))
    assert messages[-1].data.timestamp == clock.now_ns


def test_log_record():
    translator = RobbyTranslator()
    (message,) = translator.translate(robby_message("Log", {"timestamp": 0, "level": 5, "message": "boom"}))
    log = message.to_dict()["Log"]
    assert log["level"] == 4  # fatal is shown as error
    assert log["message"] == "boom"
    assert log["component"] == "robby"
    assert log["timestamp"] > 0


def test_thread_table():
    translator = RobbyTranslator()
    messages = translator.translate(robby_message("ThreadTable", {"threads": [{
        "name": "IDLE", "priority": 0, "stack_usage": 25.0, "stack_size": 128, "runtime": 80.0,
    }]}))
    dicts = [m.to_dict() for m in messages]
    assert {"ValueMetadata": {"path": "threads/IDLE/runtime", "unit": "%"}} in dicts
    values = {d["Value"]["path"]: d["Value"]["value"] for d in dicts if "Value" in d}
    assert values == {"threads/IDLE/runtime": 80.0, "threads/IDLE/stack_usage": 25.0}


@pytest.mark.parametrize("raw", [
    b"\xff\x00",
    cbor2.dumps({"other": 1}),
    robby_message("DistanceMap", {"distances_mm": []}),
    robby_message("StreamDescriptor", {"id": 0}),
])
def test_unsupported_or_malformed_messages_are_dropped(raw):
    assert RobbyTranslator().translate(raw) == []


async def test_robby_bridge():
    blip_connected = asyncio.Event()
    received_actions: asyncio.Queue[bytes] = asyncio.Queue()

    async def fake_robby(ws):
        assert ws.request.path == "/ws/robby"
        await blip_connected.wait()
        await ws.send(descriptor())
        await ws.send(frame(0, [2, 4]))
        async for message in ws:
            await received_actions.put(message)

    async with serve(fake_robby, "127.0.0.1", 3997, ping_interval=None):
        proto = RobbyProtocol(remote_host="127.0.0.1", remote_port=3997)
        server = BridgeServer(protocol=proto, host="127.0.0.1", port=3996, path="/ws/test")
        await server.start()
        try:
            async with websockets.connect("ws://127.0.0.1:3996/ws/test") as blip:
                await asyncio.sleep(0.05)
                blip_connected.set()

                metadata = decode_cbor_message(await asyncio.wait_for(blip.recv(), timeout=2.0))
                assert metadata == {"ValueMetadata": {"path": "acc/x", "unit": "g"}}
                for expected in (1.0, 2.0):
                    value = decode_cbor_message(await asyncio.wait_for(blip.recv(), timeout=2.0))
                    assert value["Value"]["path"] == "acc/x"
                    assert value["Value"]["value"] == expected

                action = cbor2.dumps({"SetMotorSpeed": 100})
                await blip.send(action)
                assert await asyncio.wait_for(received_actions.get(), timeout=2.0) == action
        finally:
            await server.stop()
