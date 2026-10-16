"""Tests for recording files and replaying them."""

import asyncio
import gzip

import cbor2
import websockets

from bridge.proto import Message
from bridge.protocols.replay import ReplayProtocol, timestamp
from bridge.recording import FORMAT, VERSION, RecordingWriter, read_frames
from bridge.server import BridgeServer


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


def test_messages_have_timestamps_except_metadata():
    assert [timestamp(frame) for frame in frames()] == [None, 1_000_000, 2_000_000, 3_000_000]


async def test_replay_sends_and_records_the_frames_unchanged(tmp_path):
    original = tmp_path / "original.blip"
    copy = tmp_path / "copy.blip"
    write(original, frames())

    proto = ReplayProtocol(original, speed=1.0)
    server = BridgeServer(
        protocol=proto, host="127.0.0.1", port=3997, path="/ws/test",
        recorder=RecordingWriter(copy),
    )
    await server.start()
    try:
        async with websockets.connect("ws://127.0.0.1:3997/ws/test") as ws:
            received = [await asyncio.wait_for(ws.recv(), timeout=2.0) for _ in frames()]
    finally:
        await server.stop()

    assert received == frames()
    assert list(read_frames(copy)) == frames()
