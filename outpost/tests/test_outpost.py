"""Tests of the outpost as a script uses it: serving from a thread, sending from any
thread, and what blips connecting later get."""

import asyncio
import socket
from typing import List

import cbor2
import pytest

import blip_outpost
from blip_outpost import Outpost
from blip_outpost.proto import Message
from blip_outpost.recording import read_frames

from blip_client import connect, messages_until, receive


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
                return await messages_until(ws, "Sample", 100)

        values = [m["Sample"]["value"] for m in asyncio.run(blip()) if "Sample" in m]
        # in the order they were sent
        assert values == list(range(100))
    finally:
        outpost.stop()
    assert not outpost.serving
    assert not outpost.wait_for_client(0.0)


def send_from_another_thread(outpost: Outpost) -> None:
    for i in range(100):
        outpost.value("count", i)


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
    outpost.unit("motor/speed_rpm", "rpm")
    outpost.value("motor/speed_rpm", 1)
    outpost.unit("imu.acc", "g")
    outpost.unit("imu.acc", "m/s²")

    @outpost.action("motor/stop")
    def stop() -> None:
        pass

    async with outpost:
        outpost.value("motor/speed_rpm", 2)
        async with connect(3972) as ws:
            first = [cbor2.loads(await receive(ws, 2.0)) for _ in range(4)]
    assert "Hello" in first[0]
    assert first[1:] == [
        {"TraceMetadata": {"path": "motor/speed_rpm", "unit": "rpm"}},
        {"TraceMetadata": {"path": "imu.acc", "unit": "m/s²"}},
        {"Actions": outpost.actions.descriptors()},
    ]


async def test_the_recording_starts_with_the_units_and_actions_sent_before(tmp_path):
    path = tmp_path / "run.blip"
    outpost = Outpost(port=3973, record=path)
    outpost.unit("speed", "rpm")
    outpost.actions.add("stop", lambda: None)
    async with outpost:
        outpost.value("speed", 1500, timestamp_ns=1)
    recorded = [cbor2.loads(frame) for frame in read_frames(path)]
    assert [next(iter(message)) for message in recorded] == ["Hello", "TraceMetadata", "Actions", "Sample"]


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
    outpost.value("speed", 1500)
    outpost.stop()
    assert [next(iter(cbor2.loads(frame))) for frame in read_frames(path)] == ["Hello", "Sample"]


def hello_of(name: str, clock: str = "epoch") -> dict:
    return {"Hello": {"version": 1, "name": name, "clock": clock, "sender": f"blip-outpost {blip_outpost.__version__}"}}


async def test_a_client_gets_a_hello_first():
    outpost = Outpost(port=3950)
    outpost.unit("a", "V")
    outpost.actions.add("stop", lambda: None)
    async with outpost:
        async with connect(3950) as ws:
            first = [cbor2.loads(await receive(ws, 2.0)) for _ in range(3)]
    assert first == [
        hello_of(socket.gethostname()),
        {"TraceMetadata": {"path": "a", "unit": "V"}},
        {"Actions": outpost.actions.descriptors()},
    ]


async def first_message(port: int) -> dict:
    async with connect(port) as ws:
        return cbor2.loads(await receive(ws, 2.0))


async def test_name_and_clock_are_settable():
    async with Outpost(port=3951, name="robby", clock="boot") as outpost:
        assert await first_message(3951) == hello_of("robby", "boot")
        outpost.name = "arm"
        assert await first_message(3951) == hello_of("arm", "boot")
        outpost.name = None
        assert await first_message(3951) == hello_of(socket.gethostname(), "boot")

    with pytest.raises(ValueError, match="epoch or boot"):
        Outpost(clock="utc")
    outpost = Outpost()
    with pytest.raises(ValueError, match="epoch or boot"):
        outpost.clock = "utc"
    assert outpost.clock == "epoch"


async def test_the_module_options_set_name_and_clock():
    outpost = blip_outpost.get_outpost()
    try:
        assert blip_outpost.serve(port=3951, name="x", clock="boot") is outpost
        assert (outpost.name, outpost.clock) == ("x", "boot")
        blip_outpost.serve(port=3951)
        assert (outpost.name, outpost.clock) == ("x", "boot")
    finally:
        outpost.name = None
        outpost.clock = "epoch"
        outpost.port = 3000


async def test_a_hello_the_script_sends_replaces_its_own():
    async with Outpost(port=3952) as outpost:
        async with connect(3952) as ws:
            assert "Hello" in cbor2.loads(await receive(ws, 2.0))
            await asyncio.to_thread(outpost.wait_for_client, 2.0)
            outpost.send(Message.create_hello(name="target", clock="boot"))
            assert cbor2.loads(await receive(ws, 2.0)) == {"Hello": {"version": 1, "name": "target", "clock": "boot"}}
        async with connect(3952) as ws:
            assert cbor2.loads(await receive(ws, 2.0)) == {"Hello": {"version": 1, "name": "target", "clock": "boot"}}
            await asyncio.to_thread(outpost.wait_for_client, 2.0)
            outpost.value("marker", 1)
            assert "Sample" in cbor2.loads(await receive(ws, 2.0))
