"""Tests of tracers, batches, series, dataclass records and the bridge from logging."""

import asyncio
import logging
from dataclasses import dataclass
from typing import Annotated, Any, List, Optional

import cbor2
import pytest
import websockets

import blip_outpost
from blip_outpost import LogHandler, LogLevel, Outpost, Unit
from blip_outpost.proto import Message

from blip_client import receive, sent


def test_the_same_name_gives_the_same_tracer():
    outpost = Outpost()
    motor = outpost.get_tracer("motor")
    assert outpost.get_tracer("motor") is motor
    assert outpost.get_tracer("/motor/") is motor
    assert motor.child("left") is outpost.get_tracer("motor/left")
    assert motor.child("left").outpost is outpost


def test_the_module_tracers_use_the_outpost_of_the_module():
    assert blip_outpost.get_tracer("motor").outpost is blip_outpost.get_outpost()


def test_a_tracer_sends_below_its_name():
    outpost = Outpost()
    messages = sent(outpost)
    motor = outpost.get_tracer("motor")
    motor.unit("speed", "rpm")
    motor.value("speed", 1432, timestamp_ns=5)
    motor.value("state", {"on": True}, timestamp_ns=6)
    outpost.get_tracer().value("cpu", 0.5, timestamp_ns=7)
    assert messages == [
        {"TraceMetadata": {"path": "motor/speed", "unit": "rpm"}},
        {"Sample": {"timestamp": 5, "path": "motor/speed", "value": 1432}},
        {"Sample": {"timestamp": 6, "path": "motor/state", "value": {"on": True}}},
        {"Sample": {"timestamp": 7, "path": "cpu", "value": 0.5}},
    ]


def test_a_tracer_logs_at_its_name():
    outpost = Outpost()
    messages = sent(outpost)
    motor = outpost.get_tracer("motor")
    motor.info("homed", timestamp_ns=1)
    motor.warn("hot: %d °C", 71, timestamp_ns=2)
    motor.warning("%s", "50%", timestamp_ns=3)
    motor.child("left").error("stalled", timestamp_ns=4)
    motor.debug("100%", timestamp_ns=5)  # no args: not formatted
    motor.log(LogLevel.TRACE, "step", timestamp_ns=6)
    assert [m["Log"] for m in messages] == [
        {"timestamp": 1, "path": "motor", "level": 2, "message": "homed"},
        {"timestamp": 2, "path": "motor", "level": 3, "message": "hot: 71 °C"},
        {"timestamp": 3, "path": "motor", "level": 3, "message": "50%"},
        {"timestamp": 4, "path": "motor/left", "level": 4, "message": "stalled"},
        {"timestamp": 5, "path": "motor", "level": 1, "message": "100%"},
        {"timestamp": 6, "path": "motor", "level": 0, "message": "step"},
    ]


def test_forwarded_logs_carry_their_path():
    outpost = Outpost()
    messages = sent(outpost)
    outpost.log(3, "hot", path="node3/motor", timestamp_ns=1)
    outpost.log(LogLevel.INFO, "booted", timestamp_ns=2)
    assert [m["Log"] for m in messages] == [
        {"timestamp": 1, "path": "node3/motor", "level": 3, "message": "hot"},
        {"timestamp": 2, "level": 2, "message": "booted"},
    ]


def test_tracer_actions_and_readbacks_are_below_its_name():
    outpost = Outpost()
    motor = outpost.get_tracer("motor")

    @motor.action("speed", readback="speed_rpm")
    def set_speed(rpm: int) -> None:
        pass

    (descriptor,) = outpost.actions.descriptors()
    assert descriptor["path"] == "motor/speed"
    assert descriptor["readback"] == "motor/speed_rpm"


@dataclass
class Pose:
    x: Annotated[float, Unit("m")]
    y: Annotated[float, Unit("m")]


@dataclass
class Imu:
    acc: Annotated[List[float], Unit("m/s²")]
    pose: Optional[Pose]
    valid: bool


def test_a_dataclass_is_a_record_whose_units_are_sent_once():
    outpost = Outpost()
    messages = sent(outpost)
    imu = Imu([0.0, 0.0, 9.81], Pose(1.0, 2.0), True)
    outpost.value("imu", imu, timestamp_ns=1)
    outpost.value("imu", imu, timestamp_ns=2)
    outpost.get_tracer("robot").value("imu", imu, timestamp_ns=3)
    record = {"acc": [0.0, 0.0, 9.81], "pose": {"x": 1.0, "y": 2.0}, "valid": True}
    assert messages == [
        {"TraceMetadata": {"path": "imu.acc", "unit": "m/s²"}},
        {"TraceMetadata": {"path": "imu.pose.x", "unit": "m"}},
        {"TraceMetadata": {"path": "imu.pose.y", "unit": "m"}},
        {"Sample": {"timestamp": 1, "path": "imu", "value": record}},
        {"Sample": {"timestamp": 2, "path": "imu", "value": record}},
        {"TraceMetadata": {"path": "robot/imu.acc", "unit": "m/s²"}},
        {"TraceMetadata": {"path": "robot/imu.pose.x", "unit": "m"}},
        {"TraceMetadata": {"path": "robot/imu.pose.y", "unit": "m"}},
        {"Sample": {"timestamp": 3, "path": "robot/imu", "value": record}},
    ]


class Array:
    """Stands in for a numpy array or scalar."""

    def __init__(self, values: Any) -> None:
        self.values = values

    def tolist(self) -> Any:
        return self.values


def test_values_with_tolist_are_sent_as_lists():
    outpost = Outpost()
    messages = sent(outpost)
    outpost.value("spectrum", Array([1.0, 2.0]), timestamp_ns=1)
    outpost.value("rec", {"gain": Array(3)}, timestamp_ns=2)
    assert [m["Sample"]["value"] for m in messages] == [[1.0, 2.0], {"gain": 3}]


def test_a_series_is_sent_at_its_timestamps():
    outpost = Outpost()
    messages = sent(outpost)
    adc = outpost.get_tracer("adc")
    adc.series("raw", Array([1, 2, 3]), start_ns=100, period_ns=2.5)
    adc.series("raw", [4, 5], timestamps_ns=Array([200, 210]))
    adc.series("raw", [6, 7, 8], start_ns=300, period_ns=50)
    assert messages == [
        {"Samples": {"path": "adc/raw", "values": [1, 2, 3], "timestamps": [100, 102, 105]}},
        {"Samples": {"path": "adc/raw", "values": [4, 5], "timestamps": [200, 210]}},
        {"Samples": {"path": "adc/raw", "values": [6, 7, 8], "timestamp": 300, "period": 50}},
    ]
    with pytest.raises(ValueError, match="2 values but 1 timestamps"):
        adc.series("raw", [1, 2], timestamps_ns=[1])
    with pytest.raises(TypeError):
        adc.series("raw", [1, 2])
    with pytest.raises(TypeError):
        adc.series("raw", [1, 2], timestamps_ns=[1, 2], start_ns=0, period_ns=1)


def test_a_batch_is_handed_over_at_its_end_with_one_timestamp():
    outpost = Outpost()
    messages = sent(outpost)
    motor = outpost.get_tracer("motor")
    with outpost.batch(timestamp_ns=50):
        motor.value("speed", 1)
        motor.info("on")
        motor.value("current", 2, timestamp_ns=7)
        with motor.batch():
            motor.value("fault", 0)
        assert messages == []
    assert [(next(iter(m)), next(iter(m.values()))["timestamp"]) for m in messages] == [
        ("Sample", 50), ("Log", 50), ("Sample", 7), ("Sample", 50),
    ]


def test_a_batch_without_a_timestamp_shares_the_first_one_needed():
    outpost = Outpost()
    messages = sent(outpost)
    with outpost.batch():
        outpost.value("a", 1)
        outpost.value("b", 2)
    a, b = (m["Sample"]["timestamp"] for m in messages)
    assert a == b > 0


def test_what_a_batch_sent_is_handed_over_when_it_raises():
    outpost = Outpost()
    messages = sent(outpost)
    with pytest.raises(RuntimeError):
        with outpost.batch(timestamp_ns=1):
            outpost.value("a", 1)
            raise RuntimeError
    assert len(messages) == 1
    # the batch is closed
    outpost.value("b", 2, timestamp_ns=2)
    assert len(messages) == 2


def test_a_batch_from_another_thread_is_one_handover():
    outpost = Outpost(port=3990).start()
    try:
        async def blip() -> List[dict]:
            async with websockets.connect("ws://127.0.0.1:3990/ws/outpost") as ws:
                assert await asyncio.to_thread(outpost.wait_for_client, 2.0)
                scheduled: List[Any] = []
                original = outpost._schedule_flush

                def schedule(loop: Any) -> None:
                    scheduled.append(loop)
                    original(loop)

                outpost._schedule_flush = schedule  # type: ignore[method-assign]

                def send() -> None:
                    with outpost.batch():
                        for i in range(100):
                            outpost.value("count", i)

                await asyncio.to_thread(send)
                assert len(scheduled) == 1
                assert "Hello" in cbor2.loads(await receive(ws))
                return [cbor2.loads(await receive(ws)) for _ in range(100)]

        values = [m["Sample"]["value"] for m in asyncio.run(blip())]
        assert values == list(range(100))
    finally:
        outpost.stop()


def test_the_log_handler_sends_the_records_of_logging():
    outpost = Outpost()
    messages = sent(outpost)
    handler = LogHandler(path="app", outpost=outpost)
    logger = logging.getLogger("test_tracer.app")
    logger.propagate = False
    logger.setLevel(1)
    logger.addHandler(handler)
    try:
        logger.debug("starting")
        logger.warning("disk %d%% full", 91)
        logger.critical("down")
        logger.log(5, "fine")
    finally:
        logger.removeHandler(handler)
    # the outpost's own records are not sent
    handler.handle(logging.makeLogRecord({"name": "blip_outpost.server", "msg": "dropping", "levelno": 30}))
    assert [(m["Log"]["level"], m["Log"]["path"], m["Log"]["message"]) for m in messages] == [
        (1, "app/test_tracer/app", "starting"),
        (3, "app/test_tracer/app", "disk 91% full"),
        (4, "app/test_tracer/app", "down"),
        (0, "app/test_tracer/app", "fine"),
    ]


def test_a_nested_batch_shares_the_timestamp_set_in_it():
    outpost = Outpost()
    messages = sent(outpost)
    with outpost.batch():
        with outpost.batch():
            outpost.value("a", 1)
        outpost.value("b", 2)
        with outpost.batch(timestamp_ns=5):
            outpost.value("c", 3)
    a, b, c = (m["Sample"]["timestamp"] for m in messages)
    assert a == b != c == 5


def test_a_single_mapping_formats_by_name_as_in_logging():
    outpost = Outpost()
    messages = sent(outpost)
    motor = outpost.get_tracer("motor")
    motor.info("speed %(rpm)d rpm", {"rpm": 1500}, timestamp_ns=1)
    motor.info("%s", {}, timestamp_ns=2)
    assert [m["Log"]["message"] for m in messages] == ["speed 1500 rpm", "{}"]


def test_units_of_fields_whose_annotations_resolve_are_sent(caplog):
    @dataclass
    class Local:
        x: float

    @dataclass
    class Sample:
        # strings, as with `from __future__ import annotations`
        speed: "Annotated[float, Unit('rpm')]"
        local: "Local"
        temperature: "Annotated[float, Unit('°C')]"

    outpost = Outpost()
    messages = sent(outpost)
    with caplog.at_level(logging.WARNING, logger="blip_outpost.records"):
        outpost.value("s", Sample(1.0, Local(2.0), 3.0), timestamp_ns=1)
    assert [m["TraceMetadata"] for m in messages if "TraceMetadata" in m] == [
        {"path": "s.speed", "unit": "rpm"},
        {"path": "s.temperature", "unit": "°C"},
    ]
    assert "local of" in caplog.text


def test_a_series_of_dataclasses_sends_their_units_once():
    outpost = Outpost()
    messages = sent(outpost)
    outpost.series("pose", [Pose(1.0, 2.0), Pose(3.0, 4.0)], start_ns=10, period_ns=5)
    assert messages == [
        {"TraceMetadata": {"path": "pose.x", "unit": "m"}},
        {"TraceMetadata": {"path": "pose.y", "unit": "m"}},
        {"Samples": {"path": "pose", "values": [{"x": 1.0, "y": 2.0}, {"x": 3.0, "y": 4.0}],
                     "timestamp": 10, "period": 5}},
    ]


def logged(handler_path: str, logger_name: str) -> str:
    outpost = Outpost()
    messages = sent(outpost)
    handler = LogHandler(path=handler_path, outpost=outpost)
    handler.handle(logging.makeLogRecord({"name": logger_name, "msg": "x", "levelno": 20}))
    return messages[0]["Log"].get("path", "")


def test_the_log_handler_sends_loggers_below_log_by_default():
    assert logged("", "app.motor") == "log/app/motor"
    assert logged("", "app") == "log/app"
    assert logged("robot", "app.motor") == "robot/app/motor"


def test_the_root_logger_is_at_the_prefix_or_the_target_as_a_whole():
    assert logged("", "root") == ""
    assert logged("robot", "root") == "robot"


def test_log_records_have_no_component():
    outpost = Outpost()
    messages = sent(outpost)
    outpost.log(2, "hi", timestamp_ns=1)
    assert messages == [{"Log": {"timestamp": 1, "level": 2, "message": "hi"}}]
    assert "component" not in cbor2.loads(Message.create_log(2, "hi", 1, "a").encode_cbor())["Log"]


def test_log_and_get_tracer_reject_a_component():
    outpost = Outpost()
    with pytest.raises(TypeError):
        outpost.log(2, "hi", component="x")  # type: ignore[call-arg]
    with pytest.raises(TypeError):
        blip_outpost.log(2, "hi", component="x")  # type: ignore[call-arg]
    with pytest.raises(TypeError):
        outpost.get_tracer("motor", component="driver")  # type: ignore[call-arg]
    with pytest.raises(TypeError):
        blip_outpost.get_tracer("motor", component="driver")  # type: ignore[call-arg]
