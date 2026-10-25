"""Integration tests for protocol to websocket outpost."""

import asyncio
import websockets

from outpost.server import OutpostServer
from outpost.protocols.simulator import SimulatorProtocol
from outpost.proto import decode_cbor_message

from blip_client import receive


async def test_simulator_outpost():
    # 1. Start server with SimulatorProtocol
    proto = SimulatorProtocol(interval_ms=10)
    server = OutpostServer(protocol=proto, host="127.0.0.1", port=3999, path="/ws/test")
    await server.start()

    await asyncio.sleep(0.05)

    try:
        # 2. Connect client to websocket
        async with websockets.connect("ws://127.0.0.1:3999/ws/test") as ws:
            # 3. Receive telemetry message (skipping the units and log records)
            for _ in range(100):
                msg_bytes = await receive(ws, 2.0)
                assert isinstance(msg_bytes, bytes)

                data = decode_cbor_message(msg_bytes)
                assert data is not None
                if "Value" in data:
                    break
            # Message structure is {"Value": {"timestamp": ..., "path": ..., "value": ...}}
            assert "Value" in data
            val_struct = data["Value"]
            assert "path" in val_struct
            assert "timestamp" in val_struct
            assert "value" in val_struct

    finally:
        await server.stop()


async def test_simulator_outpost_logs():
    proto = SimulatorProtocol(interval_ms=10, log_interval_ms=20)
    server = OutpostServer(protocol=proto, host="127.0.0.1", port=3998, path="/ws/test")
    await server.start()

    await asyncio.sleep(0.05)

    try:
        async with websockets.connect("ws://127.0.0.1:3998/ws/test") as ws:
            # Collect log records until a few have arrived
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

    finally:
        await server.stop()


async def test_simulator_outpost_typed_values():
    proto = SimulatorProtocol(interval_ms=10)
    server = OutpostServer(protocol=proto, host="127.0.0.1", port=3995, path="/ws/test")
    await server.start()

    await asyncio.sleep(0.05)

    try:
        async with websockets.connect("ws://127.0.0.1:3995/ws/test") as ws:
            # Collect the first value of each typed trace
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
                # blip accepts frames of up to 2048 bytes
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

    finally:
        await server.stop()


async def test_simulator_outpost_units():
    proto = SimulatorProtocol(interval_ms=10)
    server = OutpostServer(protocol=proto, host="127.0.0.1", port=3994, path="/ws/test")
    await server.start()

    try:
        async with websockets.connect("ws://127.0.0.1:3994/ws/test") as ws:
            # the units are repeated, so a client connecting later gets them too
            units = {}
            for _ in range(2000):
                data = decode_cbor_message(await receive(ws, 2.0))
                if "ValueMetadata" in data:
                    metadata = data["ValueMetadata"]
                    units[metadata["path"]] = metadata["unit"]
                    if "sensor/temperature" in units and "sysinfo/cpu/cpu0" in units:
                        break

            assert units["sensor/temperature"] == "°C"
            assert units["sysinfo/cpu/cpu0"] == "%"
    finally:
        await server.stop()


async def test_simulator_outpost_rates():
    proto = SimulatorProtocol(interval_ms=10)
    server = OutpostServer(protocol=proto, host="127.0.0.1", port=3993, path="/ws/test")
    await server.start()

    try:
        async with websockets.connect("ws://127.0.0.1:3993/ws/test") as ws:
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
    finally:
        await server.stop()


async def test_simulator_actions_change_what_it_sends(monkeypatch):
    import outpost.protocols.simulator as simulator
    from outpost import ActionError, LogLevel

    proto = SimulatorProtocol(interval_ms=10)
    paths = [d["path"] for d in proto.actions.descriptors()]
    assert paths == [
        "motor/enable", "motor/speed", "thermal/ambient", "log/level", "log/message", "imu/calibrate"
    ]

    await proto.invoke("motor/enable", {"enabled": False})
    assert proto._motor_message().data.value["enabled"] is False
    try:
        await proto.invoke("motor/speed", {"rpm": 1000})
        raise AssertionError("the speed of a disabled motor was set")
    except ActionError as e:
        assert "disabled" in str(e)
    await proto.invoke("motor/enable", {"enabled": True})
    await proto.invoke("motor/speed", {"rpm": 1000})
    for _ in range(1000):
        motor = proto._motor_message().data.value
    assert motor["state"] == "running" and abs(motor["speed_rpm"] - 1000) < 50

    sent = []

    async def capture(frame: bytes) -> None:
        sent.append(decode_cbor_message(frame))

    proto.attach(capture)
    await proto.invoke("log/level", {"level": "WARN"})
    assert proto._log_level == LogLevel.WARN
    assert {"path": "simulator/log_level", "value": "WARN"}.items() <= sent[-1]["Value"].items()
    await proto._send_log(LogLevel.INFO, "test", "dropped")
    await proto._send_log(LogLevel.ERROR, "test", "kept")
    assert [m["Log"]["message"] for m in sent if "Log" in m] == ["kept"]

    monkeypatch.setattr(simulator, "CALIBRATION_SEC", 0.05)
    calibration = asyncio.create_task(proto.invoke("imu/calibrate", {}))
    await asyncio.sleep(0.01)
    assert not proto.actions.is_enabled("imu/calibrate")
    await calibration
    assert proto.actions.is_enabled("imu/calibrate")
    assert proto._gyro_bias == [0.0, 0.0, 0.0]
    proto.attach(None)


if __name__ == "__main__":
    asyncio.run(test_simulator_outpost())
    asyncio.run(test_simulator_outpost_logs())
    asyncio.run(test_simulator_outpost_typed_values())
    asyncio.run(test_simulator_outpost_units())
    asyncio.run(test_simulator_outpost_rates())
    print("ALL TESTS PASSED SUCCESSFULLY!")
