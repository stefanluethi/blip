"""Tests for the socket passthrough protocol."""

import asyncio

import bridge.protocols.socket as socket_protocol
from bridge.protocols.socket import SocketProtocol


async def test_unexpected_errors_reconnect_instead_of_stopping(monkeypatch):
    attempts = []

    async def open_connection(host, port):
        attempts.append((host, port))
        raise RuntimeError("unexpected")

    monkeypatch.setattr(socket_protocol, "RETRY_DELAY_SEC", 0.01)
    monkeypatch.setattr(asyncio, "open_connection", open_connection)
    task = asyncio.create_task(SocketProtocol(remote_port=9999).run())
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
    task = asyncio.create_task(SocketProtocol(mode="udp", listen=True).run())
    await asyncio.sleep(0.2)
    try:
        assert not task.done(), "run() ended on a failing endpoint"
        assert len(attempts) >= 2
    finally:
        task.cancel()
