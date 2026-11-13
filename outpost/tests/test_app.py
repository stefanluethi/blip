"""Tests of the example app.py, a program that traces itself."""

from typing import List

import cbor2

import app
import blip_outpost


def test_the_app_traces_its_parts_below_their_names():
    outpost = blip_outpost.get_outpost()
    messages: List[dict] = []
    original = outpost._server.broadcast
    outpost._server.broadcast = lambda frame: messages.append(cbor2.loads(frame))  # type: ignore[method-assign]
    try:
        with outpost.batch():
            app.controller.step()
        app.set_setpoint(22.0)
        app.sensor.series("vibration", app.read_block(0.0), start_ns=0, period_ns=1_000_000)
    finally:
        outpost._server.broadcast = original  # type: ignore[method-assign]
    units = {m["TraceMetadata"]["path"]: m["TraceMetadata"]["unit"] for m in messages if "TraceMetadata" in m}
    assert units == {"heater/state.power": "W", "heater/state.duty": "%"}
    paths = [m["Sample"]["path"] for m in messages if "Sample" in m]
    assert paths == ["heater/state", "room/setpoint"]
    blocks = [m["Samples"] for m in messages if "Samples" in m]
    assert len(blocks) == 1
    assert blocks[0]["path"] == "room/sensor/vibration"
    assert len(blocks[0]["values"]) == 50
    assert blocks[0]["timestamp"] == 0
    assert blocks[0]["period"] == 1_000_000
    logs = [m["Log"]["path"] for m in messages if "Log" in m]
    assert "heater" in logs
