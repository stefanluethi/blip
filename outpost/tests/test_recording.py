"""Tests for recording files and replaying them."""

import asyncio
import gzip
from pathlib import Path

import cbor2
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
        Message.create_log(1, "started", timestamp_ns=3_000_000, component="motor").encode_cbor(),
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


def test_the_connection_names_of_recordings_by_blip_are_skipped(tmp_path):
    path = tmp_path / "test.blip"
    a, b, c, d = frames()
    items = [a, cbor2.dumps("robby"), b, c, cbor2.dumps(""), d]
    path.write_bytes(cbor2.dumps({"format": FORMAT, "version": 2}) + b"".join(items))
    assert list(read_frames(path)) == frames()


def test_messages_have_timestamps_except_metadata():
    assert [timestamp(frame) for frame in frames()] == [None, 1_000_000, 2_000_000, 3_000_000]


async def test_replay_sends_and_records_the_frames_unchanged(tmp_path):
    original = tmp_path / "original.blip"
    copy = tmp_path / "copy.blip"
    write(original, frames())

    async with Outpost(port=3997, path="/ws/test", record=copy) as outpost:
        replay = Replay(outpost, original, speed=1.0)
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


def test_recordings_of_sonde_are_read():
    read = list(read_frames(SONDE_RECORDING))
    assert len(read) == 12
    assert cbor2.loads(read[0]) == {"ValueMetadata": {"path": "motor/speed", "unit": "rpm"}}
    assert cbor2.loads(read[5]) == {
        "Value": {
            "timestamp": 3_000_000,
            "path": "motor",
            "value": {"speed_rpm": 1500, "enabled": True, "state": "running"},
        }
    }
    assert cbor2.loads(read[9]) == {
        "Log": {"timestamp": 3_000_000, "level": 3, "message": "no component"}
    }
    # the batches of sonde are the messages one after another
    data = SONDE_RECORDING.read_bytes()
    items, rest = split_items(data)
    assert items[1:] == read and rest == b""
