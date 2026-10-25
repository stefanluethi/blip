"""Tests for actions: their parameters, invoking them and serving them to blip."""

import asyncio
import contextlib
import enum
import threading
from typing import Annotated, Any, AsyncIterator, Dict, List, Literal, Optional

import cbor2
import pytest
import websockets
from websockets.asyncio.server import ServerConnection

from blip_outpost import ActionError, Actions, LogLevel, Outpost, Range, Unit, action

from blip_client import receive as receive_message


class Color(enum.Enum):
    RED = 1
    GREEN = 2


class Target:
    """The code of a script that talks to a target, with its actions as methods."""

    def __init__(self) -> None:
        self.calls: List[tuple] = []
        self.release = asyncio.Event()

    @action("motor/speed", readback="motor/speed_rpm")
    async def set_speed(self, rpm: Annotated[int, Range(0, 3000), Unit("rpm")]) -> None:
        """Speed setpoint

        More text that is not part of the description.
        """
        if rpm == 13:
            raise ActionError("unlucky speed")
        self.calls.append(("speed", rpm))

    @action("motor/enable")
    def enable(self, enabled: bool = True) -> None:
        self.calls.append(("enable", enabled))

    @action("reboot", description="Reboots the target", confirm=True)
    async def reboot(self) -> None:
        raise RuntimeError("flash busy")

    @action("slow")
    async def slow(self, step: int) -> None:
        if step == 0:
            await self.release.wait()
        self.calls.append(("slow", step))


def registered(target: Target, outpost: Optional[Outpost] = None) -> Outpost:
    outpost = outpost or Outpost()
    outpost.register(target)
    return outpost


def test_parameters_are_taken_from_the_signature():
    descriptors = {d["path"]: d for d in registered(Target()).actions.descriptors()}
    assert descriptors["motor/speed"] == {
        "path": "motor/speed",
        "description": "Speed setpoint",
        "params": [
            {"name": "rpm", "kind": {"Int": {"min": 0, "max": 3000}}, "unit": "rpm", "default": None}
        ],
        "readback": "motor/speed_rpm",
        "confirm": False,
        "enabled": True,
    }
    assert descriptors["motor/enable"]["params"] == [
        {"name": "enabled", "kind": "Bool", "unit": "", "default": True}
    ]
    assert descriptors["reboot"]["params"] == []
    assert descriptors["reboot"]["description"] == "Reboots the target"
    assert descriptors["reboot"]["confirm"] is True


def test_all_parameter_kinds():
    def function(
        flag: bool,
        count: int,
        ratio: Annotated[float, Range(0.0, 1.0, step=0.1)],
        mode: Literal["slow", "fast"],
        color: Color = Color.GREEN,
        name: str = "x",
    ) -> None:
        pass

    actions = Actions()
    actions.add("all", function)
    params = actions.descriptors()[0]["params"]
    assert [p["kind"] for p in params] == [
        "Bool",
        {"Int": {"min": None, "max": None}},
        {"Float": {"min": 0.0, "max": 1.0, "step": 0.1}},
        {"Enum": ["slow", "fast"]},
        {"Enum": ["RED", "GREEN"]},
        "String",
    ]
    assert params[4]["default"] == "GREEN"
    assert params[5]["default"] == "x"


@pytest.mark.parametrize("annotation", [list, Optional[int], Annotated[str, Range(0, 1)], Literal[1, 2]])
def test_unsupported_parameters_are_rejected_when_added(annotation):
    def function(value) -> None:
        pass

    function.__annotations__ = {"value": annotation}
    with pytest.raises(TypeError):
        Actions().add("bad", function)


def test_parameters_need_type_hints():
    with pytest.raises(TypeError, match="no type hint"):
        Actions().add("bad", lambda value: None)


async def test_arguments_are_checked_and_converted():
    received: Dict[str, Any] = {}

    def function(count: Annotated[int, Range(1, 5)], ratio: float = 0.5, color: Color = Color.RED) -> None:
        received.update(count=count, ratio=ratio, color=color)

    actions = Actions()
    actions.add("a", function)
    await actions.invoke("a", {"count": 2, "ratio": 1, "color": "GREEN"})
    assert received == {"count": 2, "ratio": 1.0, "color": Color.GREEN}
    assert isinstance(received["ratio"], float)

    rejected = [
        ({"count": 0}, "count: 0 is below the minimum 1"),
        ({"count": 6}, "count: 6 is above the maximum 5"),
        ({"count": 2.0}, "count: expected an integer"),
        ({"count": True}, "count: expected an integer"),
        ({"count": 2, "ratio": "x"}, "ratio: expected a number"),
        ({"count": 2, "color": "BLUE"}, "is not one of RED, GREEN"),
        ({}, "count: missing"),
        ({"count": 2, "speed": 1}, "unknown arguments: speed"),
        ([2], "not a record"),
    ]
    for args, message in rejected:
        with pytest.raises(ActionError, match=message):
            await actions.invoke("a", args)
    with pytest.raises(ActionError, match="unknown action 'b'"):
        await actions.invoke("b", {})


async def test_sync_actions_run_outside_the_event_loop():
    threads = []
    actions = Actions()
    actions.add("blocking", lambda: threads.append(threading.current_thread()))
    await actions.invoke("blocking", {})
    assert threads and threads[0] is not threading.current_thread()


async def test_disabled_actions_are_rejected():
    target = Target()
    actions = registered(target).actions
    actions.set_enabled(False, "motor/enable")
    assert actions.descriptors()[0]["enabled"] is True
    with pytest.raises(ActionError, match="disabled"):
        await actions.invoke("motor/enable", {})
    actions.set_enabled(True)
    await actions.invoke("motor/enable", {})
    assert target.calls == [("enable", True)]


async def test_exceptions_of_actions_reject_them():
    with pytest.raises(ActionError, match="RuntimeError: flash busy"):
        await registered(Target()).actions.invoke("reboot", {})


def test_changes_notify():
    changes = []
    actions = Actions(on_change=lambda: changes.append(len(actions)))
    actions.add("a", lambda: None)
    actions.set_enabled(False)
    actions.set_enabled(False)  # no change
    actions.remove("a")
    actions.remove("a")  # gone already
    assert changes == [1, 1, 0]
    with actions.batch():
        actions.add("a", lambda: None)
        actions.add("b", lambda: None)
    assert changes == [1, 1, 0, 2]


def test_functions_become_actions_and_methods_those_of_registered_objects():
    outpost = Outpost()

    @outpost.action("pump/run")
    def run_pump(seconds: float = 1.0) -> None:
        """Runs the pump"""

    class Pump:
        @outpost.action("pump/stop")
        def stop(self) -> None:
            pass

    assert [d["path"] for d in outpost.actions.descriptors()] == ["pump/run"]
    assert outpost.actions.descriptors()[0]["description"] == "Runs the pump"
    outpost.register(Pump())
    assert [d["path"] for d in outpost.actions.descriptors()] == ["pump/run", "pump/stop"]


def test_registering_announces_the_actions_once():
    outpost = Outpost()
    sent: List[dict] = []
    outpost._send_frame = lambda frame: sent.append(cbor2.loads(frame))  # type: ignore[method-assign]
    outpost.register(Target())
    assert [next(iter(message)) for message in sent] == ["Actions"]
    assert len(sent[0]["Actions"]) == 4


# The tests below serve a target and talk to it like blip


@contextlib.asynccontextmanager
async def serve(target: Target, port: int) -> AsyncIterator[Outpost]:
    async with registered(target, Outpost(port=port, path="/ws/test")) as outpost:
        yield outpost


def invoke(id: int, path: str, args: Any) -> bytes:
    return cbor2.dumps({"Invoke": {"id": id, "path": path, "args": args}})


async def receive(ws, variant: str) -> Any:
    """The content of the next message of `variant`, skipping others."""
    while True:
        message = cbor2.loads(await receive_message(ws, 2.0))
        if variant in message:
            return message[variant]


async def test_clients_get_the_actions_when_connecting():
    async with serve(Target(), 3989):
        async with websockets.connect("ws://127.0.0.1:3989/ws/test") as ws:
            actions = await receive(ws, "Actions")
            assert [a["path"] for a in actions] == ["motor/speed", "motor/enable", "reboot", "slow"]


async def test_invocations_are_answered_and_logged():
    target = Target()
    async with serve(target, 3988):
        async with websockets.connect("ws://127.0.0.1:3988/ws/test") as ws:
            await ws.send(invoke(1, "motor/speed", {"rpm": 1500}))
            assert await receive(ws, "ActionResult") == {"id": 1, "result": {"Ok": None}}
            log = await receive(ws, "Log")
            assert log["component"] == "actions"
            assert log["level"] == LogLevel.DEBUG
            assert log["message"] == "motor/speed(rpm=1500): ok"

            await ws.send(invoke(2, "motor/speed", {"rpm": 13}))
            assert await receive(ws, "ActionResult") == {"id": 2, "result": {"Err": "unlucky speed"}}
            log = await receive(ws, "Log")
            assert log["level"] == LogLevel.WARN
            assert log["message"] == "motor/speed(rpm=13) rejected: unlucky speed"

            await ws.send(invoke(3, "motor/speed", {"rpm": 4000}))
            result = await receive(ws, "ActionResult")
            assert result["result"]["Err"].startswith("rpm: 4000 is above")

            # not a request, ignored
            await ws.send(b"\x01")
            await ws.send(invoke(4, "nope", {}))
            assert await receive(ws, "ActionResult") == {"id": 4, "result": {"Err": "unknown action 'nope'"}}
        assert target.calls == [("speed", 1500)]


async def test_a_slow_action_holds_up_only_its_own_invocations():
    target = Target()
    async with serve(target, 3987):
        async with websockets.connect("ws://127.0.0.1:3987/ws/test") as ws:
            await ws.send(invoke(1, "slow", {"step": 0}))
            await ws.send(invoke(2, "slow", {"step": 1}))
            await ws.send(invoke(3, "motor/enable", {"enabled": False}))
            assert (await receive(ws, "ActionResult"))["id"] == 3
            target.release.set()
            assert (await receive(ws, "ActionResult"))["id"] == 1
            assert (await receive(ws, "ActionResult"))["id"] == 2
        assert target.calls == [("enable", False), ("slow", 0), ("slow", 1)]


def test_messages_are_told_apart_by_their_prefix():
    from blip_outpost.proto import ACTION_RESULT_PREFIX, ACTIONS_PREFIX, METADATA_PREFIX, Message

    assert Message.create_actions([]).encode_cbor().startswith(ACTIONS_PREFIX)
    assert Message.create_action_result(1, "no").encode_cbor().startswith(ACTION_RESULT_PREFIX)
    assert Message.create_metadata("a", "V").encode_cbor().startswith(METADATA_PREFIX)
    assert not Message.create_log(0, "x").encode_cbor().startswith(ACTION_RESULT_PREFIX)


async def test_answers_sent_by_the_script_are_dropped():
    from blip_outpost.proto import Message

    async with serve(Target(), 3983) as outpost:
        async with websockets.connect("ws://127.0.0.1:3983/ws/test") as ws:
            await receive(ws, "Actions")
            # e.g. a target answering a forwarded invocation with its own id
            outpost.send(Message.create_action_result(0, "not yours"))
            outpost.send_value("after", 1)
            message = cbor2.loads(await receive_message(ws, 2.0))
            assert "Value" in message


async def test_a_change_while_a_client_connects_reaches_it(monkeypatch):
    async with serve(Target(), 3984) as outpost:
        send = ServerConnection.send
        first = []

        async def changing_send(connection, message):
            # the actions change while the cached list is sent to the new client
            if not first:
                first.append(message)
                outpost.actions.set_enabled(False, "reboot")
                await asyncio.sleep(0.05)
            await send(connection, message)

        monkeypatch.setattr(ServerConnection, "send", changing_send)
        async with websockets.connect("ws://127.0.0.1:3984/ws/test") as ws:
            latest = None
            try:
                while True:
                    latest = await asyncio.wait_for(receive(ws, "Actions"), timeout=0.3)
            except asyncio.TimeoutError:
                pass
            assert latest is not None
            # the last list the client got is the changed one
            assert {a["path"]: a for a in latest}["reboot"]["enabled"] is False


async def test_changed_actions_are_announced_to_all_clients():
    async with serve(Target(), 3986) as outpost:
        async with websockets.connect("ws://127.0.0.1:3986/ws/test") as first, \
                websockets.connect("ws://127.0.0.1:3986/ws/test") as second:
            await receive(first, "Actions")
            await receive(second, "Actions")
            outpost.actions.set_enabled(False, "reboot")
            for ws in (first, second):
                actions = {a["path"]: a for a in await receive(ws, "Actions")}
                assert actions["reboot"]["enabled"] is False
            # a client connecting later gets the changed list
            async with websockets.connect("ws://127.0.0.1:3986/ws/test") as third:
                actions = {a["path"]: a for a in await receive(third, "Actions")}
                assert actions["reboot"]["enabled"] is False


async def test_results_go_to_the_invoking_client_only():
    async with serve(Target(), 3985):
        async with websockets.connect("ws://127.0.0.1:3985/ws/test") as first, \
                websockets.connect("ws://127.0.0.1:3985/ws/test") as second:
            await first.send(invoke(1, "motor/enable", {}))
            assert (await receive(first, "ActionResult"))["id"] == 1
            # the other client sees the log record, but no result
            message = cbor2.loads(await receive_message(second, 2.0))
            while "Log" not in message:
                assert "ActionResult" not in message
                message = cbor2.loads(await receive_message(second, 2.0))
