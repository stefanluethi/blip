"""Integration tests of the simulator example, connected to like blip."""

import asyncio
import contextlib
from typing import AsyncIterator

import pytest
import websockets

from blip_outpost import ActionError, LogLevel, Outpost
from blip_outpost.proto import decode_cbor_message

from blip_client import receive
from simulator import Simulator


@contextlib.asynccontextmanager
async def serving(port: int, **options) -> AsyncIterator[Simulator]:
    """The simulator running, served on `port`."""
    async with Outpost(port=port, path="/ws/test") as outpost:
        simulator = Simulator(outpost, **options)
        task = asyncio.create_task(simulator.run())
        try:
            yield simulator
        finally:
            task.cancel()
            with contextlib.suppress(asyncio.CancelledError):
                await task


def connect(port: int):
    return websockets.connect(f"ws://127.0.0.1:{port}/ws/test")


async def test_simulator_outpost():
    async with serving(3999, interval_ms=10):
        await asyncio.sleep(0.05)
        async with connect(3999) as ws:
            # the first value, after the units and log records
            for _ in range(100):
                data = decode_cbor_message(await receive(ws, 2.0))
                assert data is not None
                if "Value" in data:
                    break
            assert set(data["Value"]) == {"path", "timestamp", "value"}


async def test_simulator_outpost_logs():
    async with serving(3998, interval_ms=10, log_interval_ms=20):
        async with connect(3998) as ws:
            logs = []
            for _ in range(500):
                data = decode_cbor_message(await receive(ws, 2.0))
                assert data is not None
                if "Log" in data:
                    logs.append(data["Log"])
                    if len(logs) >= 3:
                        break

            assert len(logs) >= 3, "Simulator did not emit log records"
            for log in logs:
                assert set(log) == {"timestamp", "level", "component", "message"}
                assert isinstance(log["timestamp"], int) and log["timestamp"] > 0
                assert 0 <= log["level"] <= 4
                assert log["component"]
                assert log["message"]


async def test_simulator_outpost_typed_values():
    async with serving(3995, interval_ms=10):
        async with connect(3995) as ws:
            # the first value of each typed trace
            typed_paths = (
                "imu",
                "motor",
                "robot/pose",
                "audio/spectrum",
                "thermal/frame",
                "sensor/temperature",
            )
            values = {}
            for _ in range(500):
                msg_bytes = await receive(ws, 2.0)
                # blip accepts messages of up to 2048 bytes
                assert len(msg_bytes) <= 2048
                data = decode_cbor_message(msg_bytes)
                assert data is not None
                if "Value" in data and data["Value"]["path"] in typed_paths:
                    values.setdefault(data["Value"]["path"], data["Value"]["value"])
                    if len(values) == len(typed_paths):
                        break

    # Records and arrays arrive as plain CBOR maps and arrays
    assert set(values["imu"]) == {"acc", "gyro"}
    assert len(values["imu"]["acc"]) == 3
    assert all(isinstance(v, float) for v in values["imu"]["acc"])
    motor = values["motor"]
    assert isinstance(motor["speed_rpm"], int) and not isinstance(motor["speed_rpm"], bool)
    assert isinstance(motor["enabled"], bool)
    assert isinstance(motor["state"], str)
    pose = values["robot/pose"]
    assert set(pose) == {"x", "y", "heading"}
    assert all(isinstance(v, float) for v in pose.values())
    assert len(values["audio/spectrum"]) == 64
    frame = values["thermal/frame"]
    assert len(frame) == 8 and all(len(row) == 8 for row in frame)
    assert values["sensor/temperature"] is None or isinstance(values["sensor/temperature"], float)


async def test_simulator_outpost_units():
    async with serving(3994, interval_ms=10):
        # connecting after the simulator sent its units, it still gets them first
        await asyncio.sleep(0.1)
        async with connect(3994) as ws:
            units = {}
            data = decode_cbor_message(await receive(ws, 2.0))
            while "ValueMetadata" in data:
                units[data["ValueMetadata"]["path"]] = data["ValueMetadata"]["unit"]
                data = decode_cbor_message(await receive(ws, 2.0))

    assert units["sensor/temperature"] == "°C"
    assert units["sysinfo/cpu/cpu0"] == "%"


async def test_simulator_outpost_rates():
    async with serving(3993, interval_ms=10):
        async with connect(3993) as ws:
            # count the samples of each trace for a while
            counts = {}
            loop = asyncio.get_running_loop()
            end = loop.time() + 1.5
            while loop.time() < end:
                data = decode_cbor_message(await receive(ws, 2.0))
                if "Value" in data:
                    path = data["Value"]["path"]
                    counts[path] = counts.get(path, 0) + 1

    # every 10 ms, 50 ms, 100 ms, 250 ms and 1 s
    rates = ["accelerometer", "motor", "robot/pose", "sysinfo/cpu/cpu0", "sensor/temperature"]
    assert [counts[path] for path in rates] == sorted(
        (counts[path] for path in rates), reverse=True
    ), counts
    assert counts["accelerometer"] > 3 * counts["motor"] > 0, counts
    assert counts["robot/pose"] > 2 * counts["sensor/temperature"] > 0, counts


async def test_simulator_actions_change_what_it_sends(monkeypatch):
    import simulator as simulator_module

    outpost = Outpost()
    sent = []
    outpost._send_frame = lambda frame: sent.append(decode_cbor_message(frame))  # type: ignore[method-assign]
    proto = Simulator(outpost, interval_ms=10)
    actions = outpost.actions
    paths = [d["path"] for d in actions.descriptors()]
    assert paths == [
        "motor/enable", "motor/speed", "thermal/ambient", "log/level", "log/message", "imu/calibrate"
    ]

    await actions.invoke("motor/enable", {"enabled": False})
    assert proto._motor_message().data.value["enabled"] is False
    with pytest.raises(ActionError, match="disabled"):
        await actions.invoke("motor/speed", {"rpm": 1000})
    await actions.invoke("motor/enable", {"enabled": True})
    await actions.invoke("motor/speed", {"rpm": 1000})
    for _ in range(1000):
        motor = proto._motor_message().data.value
    assert motor["state"] == "running" and abs(motor["speed_rpm"] - 1000) < 50

    sent.clear()
    await actions.invoke("log/level", {"level": "WARN"})
    assert proto._log_level == LogLevel.WARN
    assert {"path": "simulator/log_level", "value": "WARN"}.items() <= sent[-1]["Value"].items()
    proto._send_log(LogLevel.INFO, "test", "dropped")
    proto._send_log(LogLevel.ERROR, "test", "kept")
    assert [m["Log"]["message"] for m in sent if "Log" in m] == ["kept"]

    monkeypatch.setattr(simulator_module, "CALIBRATION_SEC", 0.05)
    calibration = asyncio.create_task(actions.invoke("imu/calibrate", {}))
    await asyncio.sleep(0.01)
    assert not actions.is_enabled("imu/calibrate")
    await calibration
    assert actions.is_enabled("imu/calibrate")
    assert proto._gyro_bias == [0.0, 0.0, 0.0]
