"""Integration tests for protocol to websocket bridge."""

import asyncio
import websockets

from bridge.server import BridgeServer
from bridge.protocols.simulator import SimulatorProtocol
from bridge.proto import decode_cbor_message


async def test_simulator_bridge():
    # 1. Start server with SimulatorProtocol
    proto = SimulatorProtocol(interval_ms=10)
    server = BridgeServer(protocol=proto, host="127.0.0.1", port=3999, path="/ws/test")
    await server.start()

    await asyncio.sleep(0.05)

    try:
        # 2. Connect client to websocket
        async with websockets.connect("ws://127.0.0.1:3999/ws/test") as ws:
            # 3. Receive telemetry message (skipping interleaved log records)
            for _ in range(10):
                msg_bytes = await asyncio.wait_for(ws.recv(), timeout=2.0)
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

            # 4. Test echo message
            test_payload = b"hello_bridge_test"
            await ws.send(test_payload)
            echoed = await asyncio.wait_for(ws.recv(), timeout=2.0)

            # Might be telemetry or echo, read until we get the echo payload
            found_echo = False
            for _ in range(10):
                if echoed == test_payload:
                    found_echo = True
                    break
                echoed = await asyncio.wait_for(ws.recv(), timeout=2.0)

            assert found_echo is True, "Echo message was not received back from simulator"

    finally:
        await server.stop()


async def test_simulator_bridge_logs():
    proto = SimulatorProtocol(interval_ms=10, log_interval_ms=20)
    server = BridgeServer(protocol=proto, host="127.0.0.1", port=3998, path="/ws/test")
    await server.start()

    await asyncio.sleep(0.05)

    try:
        async with websockets.connect("ws://127.0.0.1:3998/ws/test") as ws:
            # Collect log records until a few have arrived
            logs = []
            for _ in range(500):
                data = decode_cbor_message(await asyncio.wait_for(ws.recv(), timeout=2.0))
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


if __name__ == "__main__":
    asyncio.run(test_simulator_bridge())
    asyncio.run(test_simulator_bridge_logs())
    print("ALL TESTS PASSED SUCCESSFULLY!")
