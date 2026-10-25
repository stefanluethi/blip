"""Tests of the outpost as a script uses it: serving from a thread, sending from any
thread, and what blips connecting later get."""

import asyncio
from typing import List

import cbor2
import pytest
import websockets

import blip_outpost
from blip_outpost import Outpost
from blip_outpost.recording import read_frames

from blip_client import receive


def connect(port: int):
    return websockets.connect(f"ws://127.0.0.1:{port}/ws/outpost")


async def messages_until(ws, variant: str, count: int = 1) -> List[dict]:
    """The messages received until `count` of `variant` arrived."""
    received: List[dict] = []
    while sum(variant in message for message in received) < count:
        received.append(cbor2.loads(await receive(ws, 2.0)))
    return received


def test_a_started_outpost_serves_from_its_own_thread():
    outpost = Outpost(port=3970).start()
    try:
        assert outpost.serving
        with pytest.raises(RuntimeError, match="serves already"):
            outpost.start()

        async def blip() -> List[dict]:
            async with connect(3970) as ws:
                # sent from this thread while blip is connected
                assert await asyncio.to_thread(outpost.wait_for_client, 2.0)
                await asyncio.to_thread(send_from_another_thread, outpost)
                return await messages_until(ws, "Value", 100)

        values = [m["Value"]["value"] for m in asyncio.run(blip()) if "Value" in m]
        # in the order they were sent
        assert values == list(range(100))
    finally:
        outpost.stop()
    assert not outpost.serving
    assert not outpost.wait_for_client(0.0)


def send_from_another_thread(outpost: Outpost) -> None:
    for i in range(100):
        outpost.send_value("count", i)


def test_a_port_in_use_is_reported_by_start():
    first = Outpost(port=3971).start()
    try:
        with pytest.raises(OSError):
            Outpost(port=3971).start()
    finally:
        first.stop()


async def test_units_and_actions_reach_blips_connecting_later():
    outpost = Outpost(port=3972)
    # before serving: the units and the actions are kept, the samples dropped
    outpost.send_metadata("motor/speed_rpm", "rpm")
    outpost.send_value("motor/speed_rpm", 1)
    outpost.send_metadata("imu.acc", "g")
    outpost.send_metadata("imu.acc", "m/s²")

    @outpost.action("motor/stop")
    def stop() -> None:
        pass

    async with outpost:
        outpost.send_value("motor/speed_rpm", 2)
        async with connect(3972) as ws:
            first = [cbor2.loads(await receive(ws, 2.0)) for _ in range(3)]
    assert first == [
        {"ValueMetadata": {"path": "motor/speed_rpm", "unit": "rpm"}},
        {"ValueMetadata": {"path": "imu.acc", "unit": "m/s²"}},
        {"Actions": outpost.actions.descriptors()},
    ]


async def test_the_recording_starts_with_the_units_and_actions_sent_before(tmp_path):
    path = tmp_path / "run.blip"
    outpost = Outpost(port=3973, record=path)
    outpost.send_metadata("speed", "rpm")
    outpost.actions.add("stop", lambda: None)
    async with outpost:
        outpost.send_value("speed", 1500, timestamp_ns=1)
    recorded = [cbor2.loads(frame) for frame in read_frames(path)]
    assert [next(iter(message)) for message in recorded] == ["ValueMetadata", "Actions", "Value"]


async def test_actions_may_change_from_another_thread():
    async with Outpost(port=3974) as outpost:
        outpost.actions.add("stop", lambda: None)
        async with connect(3974) as ws:
            await messages_until(ws, "Actions")
            await asyncio.to_thread(outpost.actions.set_enabled, False)
            actions = (await messages_until(ws, "Actions"))[-1]["Actions"]
            assert actions[0]["enabled"] is False


async def test_drain_waits_for_room_from_another_thread():
    async with Outpost(port=3975, max_queued=10) as outpost:
        # no blip: returns right away
        await asyncio.wait_for(asyncio.to_thread(outpost.drain), 1.0)
        with pytest.raises(RuntimeError, match="drain_async"):
            outpost.drain()
        await asyncio.wait_for(outpost.drain_async(), 1.0)


def test_the_module_functions_use_one_outpost():
    outpost = blip_outpost.get_outpost()
    assert blip_outpost.serve(port=3976) is outpost
    assert outpost.port == 3976

    @blip_outpost.action("test/module")
    def module_action() -> None:
        pass

    class Device:
        @blip_outpost.action("test/device")
        def method(self) -> None:
            pass

    try:
        assert "test/module" in outpost.actions
        assert "test/device" not in outpost.actions
        blip_outpost.register(Device())
        assert "test/device" in outpost.actions
    finally:
        outpost.actions.clear()


def test_stop_finishes_the_recording_of_a_started_outpost(tmp_path):
    path = tmp_path / "run.blip"
    outpost = Outpost(port=3977, record=path).start()
    # sent right before stopping, still recorded
    outpost.send_value("speed", 1500)
    outpost.stop()
    assert [next(iter(cbor2.loads(frame))) for frame in read_frames(path)] == ["Value"]
