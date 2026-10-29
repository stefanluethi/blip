"""Tests for translating protobuf messages to blip as their annotations say, and for the
actions of the downlink message."""

import asyncio
import json
from pathlib import Path
from typing import Any, List

import cbor2
import pytest
import websockets
from websockets.asyncio.server import serve

from blip_outpost import ActionError, Outpost
from blip_outpost.protobuf import Invoker, Translator
import blip_outpost.protolink as protolink
from blip_outpost.protolink import ProtobufLink
from blip_outpost.schema import Schema, SchemaError
from blip_client import receive as receive_message

PROTOS = Path(__file__).parent / "protos"


@pytest.fixture(scope="module")
def schema() -> Schema:
    return Schema.load([PROTOS / "test_link.proto"])


class Clock:
    """A clock the test sets, in ns."""

    def __init__(self, now: int = 0) -> None:
        self.now = now

    def __call__(self) -> int:
        return self.now


def capture() -> tuple[Outpost, List[dict]]:
    """An outpost and the messages it sends to blip, decoded."""
    sent: List[dict] = []
    outpost = Outpost()
    outpost._send_frame = lambda frame: sent.append(cbor2.loads(frame))  # type: ignore[method-assign]
    return outpost, sent


def translator(schema: Schema, **kwargs: Any) -> tuple[Translator, List[dict]]:
    outpost, sent = capture()
    return Translator(schema, "test.link.Up", outpost, **kwargs), sent


def up(schema: Schema, seq: int = 0, **item: Any) -> bytes:
    """An Up message with one item of the given kind, serialized."""
    message = schema.message_class("test.link.Up")(seq=seq)
    if item:
        message.items.add(**item)
    return message.SerializeToString()


def values(sent: List[dict], plain: bool = False) -> List[tuple]:
    """The values sent, without those of the field `plain` of each Up unless asked."""
    found = [(m["Value"]["path"], m["Value"]["value"], m["Value"]["timestamp"]) for m in sent if "Value" in m]
    return [v for v in found if plain or v[0] != "plain"]


def units(sent: List[dict]) -> dict:
    return {m["ValueMetadata"]["path"]: m["ValueMetadata"]["unit"] for m in sent if "ValueMetadata" in m}


def test_a_series_in_a_record_is_one_value_per_sample(schema):
    t, sent = translator(schema)
    imu = schema.message_class("test.link.Imu")(t_first_ns=1000, period_ns=10, x=[2, 4, 6], y=[-2, 0, 2])
    t.feed(up(schema, imu=imu))
    assert values(sent)[:3] == [
        ("imu/acc", {"x": 1.0, "y": -1.0}, 1000),
        ("imu/acc", {"x": 2.0, "y": 0.0}, 1010),
        ("imu/acc", {"x": 3.0, "y": 1.0}, 1020),
    ]
    assert units(sent) == {"imu/acc.x": "g", "imu/acc.y": "g"}


def test_a_series_without_a_record_is_a_trace_per_field_with_scaled_times(schema):
    t, sent = translator(schema)
    axes = schema.message_class("test.link.Axes")(t_us=1, period_us=2, a=[1, 2], b=[3, 4])
    t.feed(up(schema, axes=axes))
    assert values(sent)[:4] == [
        ("axes/a", 1, 1000),
        ("axes/a", 2, 3000),
        ("axes/beta", 3, 1000),
        ("axes/beta", 4, 3000),
    ]


def test_a_repeated_field_is_shaped_and_values_below_the_limit_are_null(schema):
    t, sent = translator(schema)
    image = schema.message_class("test.link.Image")(t_us=5, mm=[1, -1, 3, 4, 5, -7])
    t.feed(up(schema, image=image))
    assert values(sent)[0] == ("tof/distance", [[1, None, 3], [4, 5, None]], 5000)
    assert units(sent) == {"tof/distance": "mm"}


def test_a_shape_that_does_not_fit_sends_the_values_flat(schema, caplog):
    t, sent = translator(schema)
    image = schema.message_class("test.link.Image")(t_us=5, mm=[1, 2])
    t.feed(up(schema, image=image))
    assert values(sent)[0] == ("tof/distance", [1, 2], 5000)
    assert "do not fit the shape" in caplog.text


def test_fields_are_traces_with_their_names_units_and_scale(schema):
    t, sent = translator(schema)
    battery = schema.message_class("test.link.Battery")(t_ns=7, voltage_mv=7400, current_ma=-250, internal=9, state=1)
    t.feed(up(schema, battery=battery))
    # skipped: internal; not set: the optional temperature
    assert values(sent)[:3] == [
        ("battery/voltage", pytest.approx(7.4), 7),
        ("battery/current", -250, 7),
        ("battery/state", "Charging", 7),
    ]
    assert units(sent) == {"battery/voltage": "V", "battery/current": "mA"}


def test_an_optional_field_is_sent_when_set(schema):
    t, sent = translator(schema)
    battery = schema.message_class("test.link.Battery")(t_ns=7, temperature=0)
    t.feed(up(schema, battery=battery))
    assert ("battery/temperature", 0, 7) in values(sent)
    assert ("battery/state", "idle", 7) in values(sent)


def test_a_path_template_takes_field_values_and_units_go_once_per_path(schema):
    t, sent = translator(schema)
    task = schema.message_class("test.link.Task")
    t.feed(up(schema, task=task(t_ns=1, name="idle", runtime_bp=9950, priority=0)))
    t.feed(up(schema, task=task(t_ns=2, name="imu", runtime_bp=50, priority=3)))
    t.feed(up(schema, task=task(t_ns=3, name="idle", runtime_bp=9900, priority=0)))
    assert [v for v in values(sent) if v[0].startswith("tasks/")] == [
        ("tasks/idle", {"runtime": pytest.approx(99.5), "priority": 0}, 1),
        ("tasks/imu", {"runtime": pytest.approx(0.5), "priority": 3}, 2),
        ("tasks/idle", {"runtime": pytest.approx(99.0), "priority": 0}, 3),
    ]
    assert units(sent) == {"tasks/idle.runtime": "%", "tasks/imu.runtime": "%"}
    assert len([m for m in sent if "ValueMetadata" in m]) == 2


def test_log_records_take_level_component_and_text(schema):
    t, sent = translator(schema)
    log = schema.message_class("test.link.Log")
    t.feed(up(schema, log=log(t_ns=9, level=5, component=1, text="overflow")))
    logs = [m["Log"] for m in sent if "Log" in m]
    # blip has no FATAL
    assert logs == [{"timestamp": 9, "level": 4, "component": "imu", "message": "overflow"}]


def test_action_results_go_to_the_callback(schema):
    results = []
    t, sent = translator(schema, on_action_result=lambda id, error: results.append((id, error)))
    result = schema.message_class("test.link.Result")
    t.feed(up(schema, result=result(id=3)))
    t.feed(up(schema, result=result(id=4, error="too fast")))
    assert results == [(3, ""), (4, "too fast")]
    assert t.has_action_results


def test_messages_without_a_timestamp_get_the_estimated_time_of_the_target(schema):
    clock = Clock(1_000)
    t, sent = translator(schema, clock=clock, wall_clock=lambda: 123)
    raw = schema.message_class("test.link.Unannotated")
    # before any timestamp: the time of this computer
    t.feed(up(schema, raw=raw(a=1)))
    assert values(sent, plain=True)[:2] == [("plain", 0, 123), ("items/raw/a", 1, 123)]
    sent.clear()
    battery = schema.message_class("test.link.Battery")(t_ns=50_000)
    t.feed(up(schema, battery=battery))
    clock.now += 700
    t.feed(up(schema, raw=raw(a=2, b=[1, 2])))
    assert ("items/raw/a", 2, 50_700) in values(sent)
    assert ("items/raw/b", [1, 2], 50_700) in values(sent)
    assert t.target_time_ns() == 50_700


def test_sequence_gaps_and_restarts_are_counted(schema):
    t, _ = translator(schema)
    for seq in [5, 6, 9, 10, 0, 1, 3]:
        t.feed(up(schema, seq=seq))
    assert t.counters == {"frames": 7, "decode_errors": 0, "seq_gaps": 3, "resets": 1}
    # after a lost connection the first frame counts no gap
    t.restart()
    t.feed(up(schema, seq=100))
    assert t.counters["seq_gaps"] == 3


def test_sequence_numbers_wrap(schema):
    t, _ = translator(schema)
    for seq in [0xFFFFFFFE, 0xFFFFFFFF, 1]:
        t.feed(up(schema, seq=seq))
    assert t.counters["seq_gaps"] == 1
    assert t.counters["resets"] == 0


def test_bytes_that_are_no_message_are_counted(schema):
    t, sent = translator(schema)
    t.feed(b"\xff\xff\xff")
    assert t.counters["decode_errors"] == 1
    assert sent == []


def test_a_period_without_a_timestamp_is_an_error(tmp_path):
    proto = tmp_path / "bad.proto"
    proto.write_text(
        'syntax = "proto3"; package bad; import "blip/options.proto";\n'
        "message M { uint32 p = 1 [(blip.field).role = ROLE_PERIOD]; repeated int32 v = 2; }\n"
    )
    with pytest.raises(SchemaError, match="needs a timestamp"):
        Schema.load([proto]).plan("bad.M")


def test_a_template_naming_a_missing_field_is_an_error(tmp_path):
    proto = tmp_path / "bad.proto"
    proto.write_text(
        'syntax = "proto3"; package bad; import "blip/options.proto";\n'
        'message M { option (blip.message).path = "x/{nope}"; uint32 v = 1; }\n'
    )
    with pytest.raises(SchemaError, match="names no field 'nope'"):
        Schema.load([proto]).plan("bad.M")


def test_a_schema_that_does_not_compile_is_an_error(tmp_path):
    proto = tmp_path / "bad.proto"
    proto.write_text('syntax = "proto3"; message M { nonsense }')
    with pytest.raises(SchemaError, match="protoc failed"):
        Schema.load([proto])


def test_a_schema_without_annotations_works(tmp_path):
    proto = tmp_path / "plain.proto"
    proto.write_text('syntax = "proto3"; package plain; message M { uint32 a = 1; Sub sub = 2; }\n'
                     "message Sub { repeated float f = 1; }\n")
    schema = Schema.load([proto])
    outpost, sent = capture()
    t = Translator(schema, "plain.M", outpost, wall_clock=lambda: 5)
    m = schema.message_class("plain.M")(a=1)
    m.sub.f.extend([0.5])
    t.feed(m.SerializeToString())
    assert values(sent) == [("a", 1, 5), ("sub/f", [0.5], 5)]


# Actions


def invoker(schema: Schema, sent: List[bytes], **kwargs: Any) -> Invoker:
    async def send(payload: bytes) -> None:
        sent.append(payload)

    return Invoker(schema, "test.link.Down", send, **kwargs)


def test_the_members_of_the_invoke_oneof_are_the_actions(schema):
    outpost, _ = capture()
    invoker(schema, []).install(outpost.actions)
    descriptors = {a["path"]: a for a in outpost.actions.descriptors()}
    # blob has a repeated field
    assert list(descriptors) == ["motor/speed", "mode"]
    speed = descriptors["motor/speed"]
    assert speed["description"] == "Speed setpoint"
    assert speed["readback"] == "motor/rpm"
    assert speed["params"] == [
        {"name": "rpm", "kind": {"Int": {"min": 0, "max": 3000}}, "unit": "rpm", "default": None},
        {"name": "gain", "kind": {"Float": {"min": None, "max": None, "step": 0.5}}, "unit": "", "default": 1.5},
    ]
    mode = descriptors["mode"]
    assert mode["confirm"]
    assert mode["params"] == [
        {"name": "state", "kind": {"Enum": ["idle", "Charging"]}, "unit": "", "default": None},
        {"name": "on", "kind": "Bool", "unit": "", "default": True},
        {"name": "label", "kind": "String", "unit": "", "default": None},
    ]


async def test_an_invocation_sends_a_downlink_message_and_waits_for_the_answer(schema):
    outpost, _ = capture()
    payloads: List[bytes] = []
    inv = invoker(schema, payloads, timeout=1.0)
    inv.install(outpost.actions)

    async def answer(error: str) -> None:
        while len(payloads) < 1:
            await asyncio.sleep(0.001)
        down = schema.message_class("test.link.Down").FromString(payloads[-1])
        inv.resolve(down.invoke.id, error)

    answering = asyncio.create_task(answer(""))
    await outpost.actions.invoke("mode", {"state": "Charging", "label": "x"})
    await answering
    down = schema.message_class("test.link.Down").FromString(payloads[0])
    assert down.seq == 0
    assert down.invoke.id == 1
    assert down.invoke.WhichOneof("args") == "mode"
    assert (down.invoke.mode.state, down.invoke.mode.label, down.invoke.mode.on) == (1, "x", False)

    answering = asyncio.create_task(answer("not now"))
    with pytest.raises(ActionError, match="not now"):
        await outpost.actions.invoke("motor/speed", {"rpm": 0})
    await answering
    down = schema.message_class("test.link.Down").FromString(payloads[1])
    assert (down.seq, down.invoke.id) == (1, 2)
    # a member with only defaults is still set
    assert down.invoke.WhichOneof("args") == "speed"


async def test_an_invocation_without_an_answer_times_out(schema):
    outpost, _ = capture()
    inv = invoker(schema, [], timeout=0.05)
    inv.install(outpost.actions)
    with pytest.raises(ActionError, match="no answer"):
        await outpost.actions.invoke("motor/speed", {"rpm": 10})


async def test_without_action_results_an_invocation_succeeds_once_sent(schema):
    outpost, _ = capture()
    payloads: List[bytes] = []
    invoker(schema, payloads, wait_for_results=False).install(outpost.actions)
    await outpost.actions.invoke("motor/speed", {"rpm": 10})
    assert len(payloads) == 1


async def test_invalid_arguments_are_rejected_before_sending(schema):
    outpost, _ = capture()
    payloads: List[bytes] = []
    invoker(schema, payloads).install(outpost.actions)
    with pytest.raises(ActionError, match="above the maximum"):
        await outpost.actions.invoke("motor/speed", {"rpm": 3001})
    assert payloads == []


# The WebSocket of the target


async def test_the_link_translates_the_target_and_forwards_actions(schema):
    received: List[bytes] = []
    result = schema.message_class("test.link.Result")
    battery = schema.message_class("test.link.Battery")

    async def target(ws):
        await ws.send(json.dumps({"units": {"bridge/rx": "B"}, "values": {"bridge/rx": 10}}))
        await ws.send(up(schema, seq=0, battery=battery(t_ns=1_000, voltage_mv=8000)))
        async for frame in ws:
            received.append(frame)
            down = schema.message_class("test.link.Down").FromString(frame)
            await ws.send(up(schema, seq=1, result=result(id=down.invoke.id)))

    async with serve(target, "127.0.0.1", 3961):
        async with Outpost(port=3962, path="/ws/test") as outpost:
            link = ProtobufLink(outpost, schema, "ws://127.0.0.1:3961/ws/robby", "test.link.Up", "test.link.Down",
                                stats_period=0.05)
            running = asyncio.create_task(link.run())
            try:
                async with websockets.connect("ws://127.0.0.1:3962/ws/test") as blip:
                    got: dict = {}
                    while "battery/voltage" not in got or "bridge/rx" not in got:
                        message = cbor2.loads(await receive_message(blip))
                        if "Value" in message:
                            got[message["Value"]["path"]] = message["Value"]
                    assert got["battery/voltage"]["value"] == pytest.approx(8.0)
                    assert got["bridge/rx"]["value"] == 10

                    await blip.send(cbor2.dumps({"Invoke": {"id": 77, "path": "motor/speed", "args": {"rpm": 5}}}))
                    while True:
                        message = cbor2.loads(await receive_message(blip))
                        if "ActionResult" in message:
                            break
                    assert message["ActionResult"] == {"id": 77, "result": {"Ok": None}}
                    assert len(received) == 1

                    while "outpost/frames" not in got:
                        message = cbor2.loads(await receive_message(blip))
                        if "Value" in message:
                            got[message["Value"]["path"]] = message["Value"]
                    assert got["outpost/frames"]["value"] >= 2
            finally:
                running.cancel()


async def test_actions_are_disabled_while_the_target_is_not_connected(schema):
    async with Outpost(port=3963, path="/ws/test") as outpost:
        # nothing listens there
        ProtobufLink(outpost, schema, "ws://127.0.0.1:9/ws/robby", "test.link.Up", "test.link.Down")
        assert not outpost.actions.is_enabled("motor/speed")
        with pytest.raises(ActionError, match="disabled"):
            await outpost.actions.invoke("motor/speed", {"rpm": 1})


async def test_a_target_that_falls_silent_is_connected_again(schema, monkeypatch):
    monkeypatch.setattr(protolink, "RETRY_DELAY_SEC", 0.05)
    connections = []

    async def target(ws):
        connections.append(ws)
        await ws.send(up(schema, seq=0))
        # silent, without closing: like a target that restarted
        await asyncio.sleep(1)

    async with serve(target, "127.0.0.1", 3964):
        async with Outpost(port=3965, path="/ws/test") as outpost:
            link = ProtobufLink(outpost, schema, "ws://127.0.0.1:3964/ws/robby", "test.link.Up", idle_timeout=0.2)
            running = asyncio.create_task(link.run())
            try:
                for _ in range(100):
                    if len(connections) >= 2:
                        break
                    await asyncio.sleep(0.05)
                assert len(connections) >= 2
                assert link.translator.counters["frames"] >= 2
            finally:
                running.cancel()
