"""Tests of blocks of samples: the Samples message and `Outpost.series`."""


import cbor2
import pytest

from blip_outpost import Outpost
from blip_outpost.proto import Message

from blip_client import connect, receive, sent


def test_a_block_has_a_period_or_timestamps():
    periodic = Message.create_samples("adc", [1, 2, 3], timestamp_ns=1000, period_ns=500)
    assert periodic.to_dict() == {
        "Samples": {"path": "adc", "values": [1, 2, 3], "timestamp": 1000, "period": 500}}
    listed = Message.create_samples("adc", [1, 2, 3], timestamps_ns=[1, 5, 9])
    assert listed.to_dict() == {"Samples": {"path": "adc", "values": [1, 2, 3], "timestamps": [1, 5, 9]}}
    for message in (periodic, listed):
        assert cbor2.loads(message.encode_cbor()) == message.to_dict()
        keys = message.to_dict()["Samples"].keys()
        assert not ({"timestamps"} <= keys and ({"timestamp"} & keys or {"period"} & keys))


def test_a_malformed_block_is_refused():
    with pytest.raises(ValueError, match="either timestamps"):
        Message.create_samples("a", [1])
    with pytest.raises(ValueError, match="either timestamps"):
        Message.create_samples("a", [1], timestamps_ns=[1], period_ns=1)
    with pytest.raises(ValueError, match="either timestamps"):
        Message.create_samples("a", [1], period_ns=1)
    with pytest.raises(ValueError, match="either timestamps"):
        Message.create_samples("a", [1], timestamp_ns=1)
    with pytest.raises(ValueError, match="3 values but 2 timestamps"):
        Message.create_samples("a", [1, 2, 3], timestamps_ns=[1, 2])
    with pytest.raises(ValueError, match="negative"):
        Message.create_samples("a", [1], timestamp_ns=-1, period_ns=1)
    with pytest.raises(ValueError, match="negative"):
        Message.create_samples("a", [1, 2], timestamps_ns=[1, -2])


async def test_a_script_hands_a_block_over_as_one_message():
    async with Outpost(port=3955) as outpost:
        async with connect(3955) as ws:
            outpost.series("adc", list(range(100)), start_ns=1_000_000, period_ns=1000)
            outpost.series("adc", list(range(100)), start_ns=1_000_000, period_ns=1000.0)
            outpost.series("adc", [], start_ns=1, period_ns=1)
            outpost.value("marker", 1, timestamp_ns=1)
            received = []
            while not any("Sample" in m and m["Sample"]["path"] == "marker" for m in received):
                received.append(cbor2.loads(await receive(ws, 2.0)))
    blocks = [m for m in received if "Samples" in m]
    expected = {"path": "adc", "values": list(range(100)), "timestamp": 1_000_000, "period": 1000}
    assert [m["Samples"] for m in blocks] == [expected, expected]
    # nothing else of adc, not even for the empty block
    assert [m["Sample"]["path"] for m in received if "Sample" in m] == ["marker"]
    first = next(i for i, m in enumerate(received) if "Samples" in m)
    assert not any("Sample" in m and m["Sample"]["path"] == "adc" for m in received[:first])


def test_a_batch_hands_a_block_over_with_its_messages():
    outpost = Outpost()
    messages = sent(outpost)
    with outpost.batch(timestamp_ns=5):
        outpost.value("a", 1)
        outpost.series("b", [1, 2], timestamps_ns=[7, 8])
        outpost.value("c", 3)
        assert messages == []
    assert [next(iter(m)) for m in messages] == ["Sample", "Samples", "Sample"]
