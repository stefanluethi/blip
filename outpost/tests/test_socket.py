"""Tests for the socket passthrough protocol."""

import asyncio
import socket

import blip_outpost.passthrough as socket_protocol
from blip_outpost import Outpost
from blip_outpost.passthrough import SocketPassthrough
from blip_outpost.proto import Message
from blip_outpost.recording import read_frames


def messages() -> list[bytes]:
    return [
        Message.create_metadata("speed", "rpm").encode_cbor(),
        Message.create_value("speed", 1500, timestamp_ns=1_000_000).encode_cbor(),
        Message.create_value("imu", {"acc": [0.5, -1.0]}, timestamp_ns=2_000_000).encode_cbor(),
        Message.create_log(1, "started", timestamp_ns=3_000_000, component="motor").encode_cbor(),
    ]


class Reads:
    """A stream reader returning the given reads, then the end of the stream."""

    def __init__(self, *reads: bytes) -> None:
        self.reads = list(reads)

    async def read(self, n: int) -> bytes:
        return self.reads.pop(0) if self.reads else b""


def passthrough(**options) -> tuple[SocketPassthrough, list[bytes]]:
    """A passthrough and the frames it sends to blip, as they arrive."""
    frames: list[bytes] = []
    outpost = Outpost()
    outpost._send_frame = frames.append  # type: ignore[method-assign]
    return SocketPassthrough(outpost, **options), frames


async def test_several_messages_of_one_read_are_sent_one_by_one():
    protocol, frames = passthrough()
    await protocol._forward(Reads(b"".join(messages())))
    assert frames == messages()


async def test_messages_split_over_reads_are_sent_once_complete():
    protocol, frames = passthrough()
    data = b"".join(messages())
    # within the first message, and within the third
    cuts = [3, len(messages()[0]) + len(messages()[1]) + 5]
    await protocol._forward(Reads(data[:cuts[0]], data[cuts[0]:cuts[1]], data[cuts[1]:]))
    assert frames == messages()


async def test_an_incomplete_message_at_the_end_of_the_stream_is_dropped():
    protocol, frames = passthrough()
    first, second = messages()[:2]
    await protocol._forward(Reads(first + second[:-1]))
    assert frames == [first]


async def test_undecodable_data_is_dropped_and_later_messages_are_sent():
    protocol, frames = passthrough()
    first, second = messages()[:2]
    # 0x1c is no CBOR
    await protocol._forward(Reads(bytes.fromhex("1c") + first, second))
    assert frames == [second]


async def test_a_message_that_does_not_end_is_dropped(monkeypatch):
    monkeypatch.setattr(socket_protocol, "MAX_PENDING_BYTES", 100)
    protocol, frames = passthrough()
    # a byte string of 2^32 bytes
    await protocol._forward(Reads(bytes.fromhex("5a ffffffff") + bytes(200), messages()[0]))
    assert frames == [messages()[0]]


async def test_several_messages_of_one_datagram_are_sent_one_by_one():
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    protocol, frames = passthrough(mode="udp", port=port, listen=True)
    task = asyncio.create_task(protocol.run())
    try:
        for _ in range(100):
            if protocol._udp_transport is not None:
                break
            await asyncio.sleep(0.01)
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as target:
            target.sendto(b"".join(messages()), ("127.0.0.1", port))
        for _ in range(100):
            if len(frames) == len(messages()):
                break
            await asyncio.sleep(0.01)
        assert frames == messages()
    finally:
        task.cancel()


async def test_actions_and_action_results_within_a_batch_are_told_apart(tmp_path):
    path = tmp_path / "batch.blip"
    actions = Message.create_actions([{"path": "motor/stop", "enabled": True}]).encode_cbor()
    result = Message.create_action_result(7).encode_cbor()
    value, log = messages()[1], messages()[3]

    async with Outpost(port=3969, record=path) as outpost:
        await SocketPassthrough(outpost)._forward(Reads(value + actions + result + log))
        # kept for clients connecting later, while the result of the target is dropped
        assert outpost._server._actions_frame == actions
    assert list(read_frames(path)) == [value, actions, log]


async def test_invocations_are_forwarded_to_the_target():
    outpost = Outpost()
    protocol = SocketPassthrough(outpost)
    assert outpost.on_invoke == protocol.invoke


async def test_unexpected_errors_reconnect_instead_of_stopping(monkeypatch):
    attempts = []

    async def open_connection(host, port):
        attempts.append((host, port))
        raise RuntimeError("unexpected")

    monkeypatch.setattr(socket_protocol, "RETRY_DELAY_SEC", 0.01)
    monkeypatch.setattr(asyncio, "open_connection", open_connection)
    task = asyncio.create_task(SocketPassthrough(Outpost(), port=9999).run())
    await asyncio.sleep(0.2)
    try:
        assert not task.done(), "run() ended on an unexpected error"
        assert len(attempts) >= 2
    finally:
        task.cancel()


async def test_failing_udp_endpoints_are_retried(monkeypatch):
    attempts = []
    loop = asyncio.get_running_loop()

    async def create_datagram_endpoint(*args, **kwargs):
        attempts.append(kwargs)
        raise OSError("address in use")

    monkeypatch.setattr(socket_protocol, "RETRY_DELAY_SEC", 0.01)
    monkeypatch.setattr(loop, "create_datagram_endpoint", create_datagram_endpoint)
    task = asyncio.create_task(SocketPassthrough(Outpost(), mode="udp", listen=True).run())
    await asyncio.sleep(0.2)
    try:
        assert not task.done(), "run() ended on a failing endpoint"
        assert len(attempts) >= 2
    finally:
        task.cancel()
