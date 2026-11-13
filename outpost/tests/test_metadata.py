"""Tests of the metadata of traces: description, labels and range."""

import asyncio
from dataclasses import dataclass
from typing import Annotated, List, Optional

import cbor2
import pytest
import websockets

import blip_outpost
from blip_outpost import Description, Labels, Outpost, Range, Unit

from blip_client import messages_until, sent


@dataclass
class Pose:
    x: Annotated[float, Unit("m")]
    y: Annotated[float, Unit("m")]


@dataclass
class Drive:
    speed: Annotated[int, Unit("rpm"), Range(0, 3000), Description("Shaft speed")]
    state: Annotated[int, Labels({0: "idle", 1: "running"})]
    limit: Annotated[float, Range(min=0.0)]
    ok: bool
    pose: Annotated[Optional[Pose], Description("Where")]


def test_a_trace_declares_description_labels_and_range():
    outpost = Outpost()
    messages = sent(outpost)
    outpost.metadata("motor/state", description="State of the drive", labels={0: "idle", 1: "running", 2: "fault"})
    outpost.metadata("temp", "°C", value_range=(15, 35.5))
    assert messages == [
        {"TraceMetadata": {"path": "motor/state", "description": "State of the drive",
                           "enum": {0: "idle", 1: "running", 2: "fault"}}},
        {"TraceMetadata": {"path": "temp", "unit": "°C", "range": {"min": 15, "max": 35.5}}},
    ]


def test_calls_merge_per_path():
    outpost = Outpost()
    messages = sent(outpost)
    outpost.unit("temp", "°C")
    outpost.metadata("temp", description="Room", value_range=(15, 35))
    outpost.unit("temp", "")
    outpost.metadata("temp", description="")
    rng = {"min": 15, "max": 35}
    assert [m["TraceMetadata"] for m in messages] == [
        {"path": "temp", "unit": "°C"},
        {"path": "temp", "unit": "°C", "description": "Room", "range": rng},
        {"path": "temp", "description": "Room", "range": rng},
        {"path": "temp", "range": rng},
    ]


def test_metadata_without_the_optional_fields_is_as_before():
    outpost = Outpost()
    messages = sent(outpost)
    outpost.unit("a", "V")
    assert messages == [{"TraceMetadata": {"path": "a", "unit": "V"}}]
    outpost.get_tracer("motor").metadata("speed", "rpm", description="x")
    assert messages[1] == {"TraceMetadata": {"path": "motor/speed", "unit": "rpm", "description": "x"}}


@pytest.mark.parametrize("kwargs", [
    {"labels": {"a": "b"}},
    {"labels": {1: 2}},
    {"labels": {True: "x"}},
    {"value_range": (5, 1)},
    {"value_range": (0, "a")},
    {"value_range": (1,)},
])
def test_bad_metadata_is_refused(kwargs):
    outpost = Outpost()
    messages = sent(outpost)
    with pytest.raises(ValueError):
        outpost.metadata("a", **kwargs)
    assert messages == []


def test_the_fields_of_a_dataclass_declare_metadata():
    outpost = Outpost()
    messages = sent(outpost)
    drive = Drive(1200, 1, 0.5, True, Pose(1.0, 2.0))
    outpost.value("drive", drive, timestamp_ns=1)
    outpost.value("drive", drive, timestamp_ns=2)
    outpost.value("robot/drive", drive, timestamp_ns=3)

    def declared(prefix: str) -> List[dict]:
        return [
            {"TraceMetadata": {"path": f"{prefix}.speed", "unit": "rpm", "description": "Shaft speed",
                               "range": {"min": 0, "max": 3000}}},
            {"TraceMetadata": {"path": f"{prefix}.state", "enum": {0: "idle", 1: "running"}}},
            {"TraceMetadata": {"path": f"{prefix}.pose", "description": "Where"}},
            {"TraceMetadata": {"path": f"{prefix}.pose.x", "unit": "m"}},
            {"TraceMetadata": {"path": f"{prefix}.pose.y", "unit": "m"}},
        ]

    kinds = [next(iter(m)) for m in messages]
    assert kinds == ["TraceMetadata"] * 5 + ["Sample", "Sample"] + ["TraceMetadata"] * 5 + ["Sample"]
    assert messages[:5] == declared("drive")
    assert messages[7:12] == declared("robot/drive")


def test_the_module_function_sends_metadata(monkeypatch):
    messages: List[dict] = []
    # undone after the test, as the outpost is the global one
    monkeypatch.setattr(
        blip_outpost.get_outpost()._server, "broadcast", lambda frame: messages.append(cbor2.loads(frame))
    )
    blip_outpost.metadata("module/trace", "V", description="d")
    assert messages[-1] == {"TraceMetadata": {"path": "module/trace", "unit": "V", "description": "d"}}


@pytest.mark.asyncio
async def test_clients_connecting_later_get_the_merged_metadata_once():
    outpost = Outpost(port=3954)
    outpost.unit("a", "V")
    outpost.metadata("a", description="x")
    async with outpost:
        async with websockets.connect("ws://127.0.0.1:3954/ws/outpost") as ws:
            received = await messages_until(ws, "TraceMetadata", 1)
            assert [m["TraceMetadata"] for m in received if "TraceMetadata" in m] == [
                {"path": "a", "unit": "V", "description": "x"}
            ]
            await asyncio.to_thread(outpost.value, "marker", 1)
            more = await messages_until(ws, "Sample", 1)
            assert not any("TraceMetadata" in m for m in more)
