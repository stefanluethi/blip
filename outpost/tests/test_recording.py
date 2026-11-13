"""Tests for recording files and replaying them."""

import asyncio
import gzip
from pathlib import Path

import cbor2
import pytest
import websockets

from blip_outpost import Outpost
from blip_outpost.proto import Message, split_items
from blip_outpost.recording import FORMAT, VERSION, RecordingWriter, read_frames
from blip_outpost.replay import Replay, timestamp

from blip_client import receive


def frames() -> list[bytes]:
    return [
        Message.create_metadata("speed", "rpm").encode_cbor(),
        Message.create_value("speed", 1500, timestamp_ns=1_000_000).encode_cbor(),
        Message.create_value("imu", {"acc": [0.5, -1.0]}, timestamp_ns=2_000_000).encode_cbor(),
        Message.create_log(1, "started", timestamp_ns=3_000_000, path="log/motor").encode_cbor(),
    ]


def write(path, frames: list[bytes]) -> None:
    writer = RecordingWriter(path)
    for frame in frames:
        writer.write(frame)
    writer.close()


def test_recordings_are_a_header_and_the_frames(tmp_path):
    path = tmp_path / "test.blip"
    write(path, frames())

    data = gzip.decompress(path.read_bytes())
    assert cbor2.loads(data) == {"format": FORMAT, "version": VERSION}
    assert list(read_frames(path)) == frames()


def test_a_cut_off_recording_is_read_up_to_the_cut(tmp_path):
    path = tmp_path / "test.blip"
    many = frames() * 500
    write(path, many)
    data = path.read_bytes()
    path.write_bytes(data[: len(data) // 2])

    read = list(read_frames(path))
    assert 0 < len(read) < len(many)
    assert read == many[: len(read)]


def test_uncompressed_recordings_are_read(tmp_path):
    path = tmp_path / "test.blip"
    path.write_bytes(cbor2.dumps({"format": FORMAT, "version": VERSION}) + b"".join(frames()))
    assert list(read_frames(path)) == frames()


@pytest.mark.parametrize("version", [2, 3])
def test_the_connection_names_of_recordings_by_blip_are_skipped(tmp_path, version):
    path = tmp_path / "test.blip"
    a, b, c, d = frames()
    items = [a, cbor2.dumps("robby"), b, c, cbor2.dumps(""), d]
    path.write_bytes(cbor2.dumps({"format": FORMAT, "version": version}) + b"".join(items))
    assert list(read_frames(path)) == frames()


def test_the_outpost_writes_version_3_without_connection_names(tmp_path):
    path = tmp_path / "test.blip"
    write(path, frames())
    assert VERSION == 3
    raw = path.read_bytes()
    assert raw.startswith(b"\x1f\x8b")
    items, rest = split_items(gzip.decompress(raw))
    assert rest == b""
    assert cbor2.loads(items[0]) == {"format": "blip recording", "version": 3}
    assert all(isinstance(cbor2.loads(item), dict) for item in items[1:])
    assert list(read_frames(path)) == frames()


@pytest.mark.parametrize("compressed", [True, False])
@pytest.mark.parametrize("version", [1, 2, 3])
def test_versions_1_to_3_are_read(tmp_path, version, compressed):
    path = tmp_path / "test.blip"
    items = frames()
    if version >= 2:
        items = [item for frame in items for item in (frame, cbor2.dumps("robby"))]
    data = cbor2.dumps({"format": FORMAT, "version": version}) + b"".join(items)
    path.write_bytes(gzip.compress(data) if compressed else data)
    assert list(read_frames(path)) == frames()


def test_a_newer_version_is_refused(tmp_path):
    path = tmp_path / "test.blip"
    path.write_bytes(cbor2.dumps({"format": FORMAT, "version": 4}))
    with pytest.raises(ValueError, match="reads up to 3"):
        list(read_frames(path))


OLD_FRAMES = [cbor2.dumps(m) for m in [
    {"ValueMetadata": {"path": "speed", "unit": "rpm"}},
    {"Value": {"timestamp": 1_000_000, "path": "speed", "value": 1500}},
    {"Log": {"timestamp": 3_000_000, "path": "log/motor", "level": 1, "message": "started"}},
]]
NEW_FRAMES = [cbor2.dumps(m) for m in [
    {"Hello": {"version": 1, "name": "robby", "clock": "boot"}},
    {"TraceMetadata": {"path": "speed", "unit": "rpm"}},
    {"Sample": {"timestamp": 1_000_000, "path": "speed", "value": 1500}},
    {"Samples": {"path": "adc", "values": [1, 2], "timestamp": 2_000_000, "period": 1000}},
    {"Samples": {"path": "adc", "values": [3, 4], "timestamps": [3_000_000, 3_001_000]}},
    {"Log": {"timestamp": 3_000_000, "path": "log/motor", "level": 1, "message": "started"}},
]]


@pytest.mark.parametrize("version, port", [(1, 3958), (2, 3959), (3, 3960)])
async def test_the_messages_of_every_version_are_replayed_unchanged(tmp_path, version, port):
    expected = OLD_FRAMES if version == 1 else NEW_FRAMES
    original = tmp_path / "original.blip"
    copy = tmp_path / "copy.blip"
    items = expected
    if version == 2:
        items = [item for frame in expected for item in (cbor2.dumps("robby"), frame)]
    original.write_bytes(gzip.compress(cbor2.dumps({"format": FORMAT, "version": version}) + b"".join(items)))

    outpost = Outpost(port=port, path="/ws/test", record=copy)
    replay = Replay(outpost, original, speed=1.0)
    async with outpost:
        task = asyncio.create_task(replay.run())
        try:
            async with websockets.connect(f"ws://127.0.0.1:{port}/ws/test") as ws:
                received = [await receive(ws, 2.0) for _ in expected]
            await asyncio.wait_for(task, 2.0)
        finally:
            task.cancel()

    assert received == expected
    assert list(read_frames(copy)) == expected


def test_messages_have_timestamps_except_metadata():
    assert [timestamp(frame) for frame in frames()] == [None, 1_000_000, 2_000_000, 3_000_000]
    period = Message.create_samples("adc", [1, 2], timestamp_ns=2_000_000, period_ns=1000).encode_cbor()
    listed = Message.create_samples("adc", [1, 2], timestamps_ns=[3_000_000, 3_001_000]).encode_cbor()
    assert timestamp(period) == 2_000_000
    assert timestamp(listed) == 3_000_000
    assert timestamp(Message.create_hello(name="a").encode_cbor()) is None


async def test_replay_sends_and_records_the_frames_unchanged(tmp_path):
    original = tmp_path / "original.blip"
    copy = tmp_path / "copy.blip"
    write(original, frames())

    outpost = Outpost(port=3997, path="/ws/test", record=copy)
    # created before serving, so the outpost has no Hello of its own in the recording
    replay = Replay(outpost, original, speed=1.0)
    async with outpost:
        task = asyncio.create_task(replay.run())
        try:
            async with websockets.connect("ws://127.0.0.1:3997/ws/test") as ws:
                received = [await receive(ws, 2.0) for _ in frames()]
            await asyncio.wait_for(task, 2.0)
        finally:
            task.cancel()

    assert received == frames()
    assert list(read_frames(copy)) == frames()


# a recording written by sonde, the C++ tracing library, uncompressed
SONDE_RECORDING = Path(__file__).parents[2] / "sonde" / "tests" / "data" / "recording.blip"


RENAMED = {"Value": "Sample", "ValueMetadata": "TraceMetadata"}


def decoded(frame: bytes) -> dict:
    """A message with the old names of the variants mapped to the new."""
    message = cbor2.loads(frame)
    return {RENAMED.get(name, name): body for name, body in message.items()}


def test_recordings_of_sonde_are_read():
    read = list(read_frames(SONDE_RECORDING))
    messages = [decoded(frame) for frame in read]
    assert {"TraceMetadata": {"path": "motor/speed", "unit": "rpm"}} in messages
    assert {
        "Sample": {
            "timestamp": 3_000_000,
            "path": "motor",
            "value": {"speed_rpm": 1500, "enabled": True, "state": "running"},
        }
    } in messages
    arm = {"Log": {"timestamp": 3_000_000, "path": "robot/arm", "level": 2, "message": "started"}}
    # sonde leaves out an empty path, as the outpost does
    no_path = {"Log": {"timestamp": 3_000_000, "level": 3, "message": "no path"}}
    assert arm in messages
    assert no_path in messages
    assert cbor2.loads(Message.create_log(3, "no path", 3_000_000).encode_cbor()) == no_path
    assert cbor2.loads(Message.create_log(2, "started", 3_000_000, "robot/arm").encode_cbor()) == arm
    # the batches of sonde are the messages one after another
    data = SONDE_RECORDING.read_bytes()
    items, rest = split_items(data)
    assert items[1:] == read and rest == b""
