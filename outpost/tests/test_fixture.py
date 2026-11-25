"""The fixture of the messages the outpost writes, which the tests of blip-proto check
against the schema of the protocol (`proto/blip.cddl`, `proto/tests/schema.rs`)."""

import enum
import os
from pathlib import Path
from typing import Annotated, Literal

from blip_outpost import Actions, Range, Unit
from blip_outpost.proto import Message

FIXTURE = Path(__file__).parent / "data" / "messages.cbor"


class Mode(enum.Enum):
    IDLE = 0
    RUN = 1


def set_speed(rpm: Annotated[int, Range(0, 3000), Unit("rpm")] = 1500) -> None:
    """Speed setpoint"""


def set_gain(gain: Annotated[float, Range(0.0, 2.0, step=0.25)], label: str, on: bool) -> None:
    pass


def set_mode(mode: Mode, log: Literal["quiet", "loud"] = "quiet", offset: int = 0) -> None:
    pass


def reboot() -> None:
    pass


def messages() -> list[bytes]:
    """A message of each kind, with and without its optional fields."""
    actions = Actions()
    actions.add("motor/speed", set_speed, readback="motor/speed_rpm")
    actions.add("motor/gain", set_gain)
    actions.add("motor/mode", set_mode)
    actions.add("reboot", reboot, description="Reboots the target", confirm=True)
    actions.set_enabled(False, "reboot", reason="calibrating")
    ms = 1_000_000
    made = [
        Message.create_hello(name="rig-7", clock="epoch", sender="blip-outpost 0.1.0"),
        Message.create_hello(),
        Message.create_metadata("motor/speed", "rpm"),
        Message.create_metadata("motor/idle"),
        Message.create_metadata(
            "motor/mode",
            description="Operating mode",
            labels={-1: "fault", 0: "idle", 1: "run"},
            value_range=(-1, 1.5),
        ),
        Message.create_value("motor/speed", 1500, ms),
        Message.create_value("offset", -42, ms),
        Message.create_value("cpu_load", 12.5, ms),
        Message.create_value("ratio", 0.1, ms),
        Message.create_value("sensor/temperature", None, ms),
        Message.create_value("state", "running", ms),
        Message.create_value("imu", {"acc": [0.5, -1.0, 9.81], "valid": True, "inner": {"n": 1}}, ms),
        Message.create_samples("adc/raw", [512, 513, 511], timestamp_ns=2 * ms, period_ns=250_000),
        Message.create_samples("adc/peak", [1.5, None], timestamps_ns=[2 * ms, 2 * ms + 500_000]),
        Message.create_samples("adc/none", [], timestamps_ns=[]),
        Message.create_log(2, "started", 3 * ms, path="robot/arm"),
        Message.create_log(4, "no path", 0),
        Message.create_actions(actions.descriptors()),
        Message.create_actions([]),
        Message.create_action_result(1),
        Message.create_action_result(2, "unlucky speed"),
    ]
    return [message.encode_cbor() for message in made]


def test_the_fixture_is_what_the_outpost_writes():
    """Run with OUTPOST_UPDATE_FIXTURES=1 to write the fixture anew."""
    written = b"".join(messages())
    if os.environ.get("OUTPOST_UPDATE_FIXTURES") == "1":
        FIXTURE.write_bytes(written)
    assert FIXTURE.read_bytes() == written
