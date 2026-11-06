"""Tests for keeping the time of a target going forward when it restarts."""

import cbor2

from blip_outpost.proto import Message, retime
from blip_outpost.timing import TargetClock

SECOND = 1_000_000_000


class Clock:
    """A clock the test sets, in ns."""

    def __init__(self, now: int = 0) -> None:
        self.now = now

    def __call__(self) -> int:
        return self.now


def test_the_time_of_a_target_is_kept_until_it_restarts():
    clock = Clock()
    target = TargetClock(clock)
    assert target.now() is None
    assert [target.map(t) for t in [5 * SECOND, 6 * SECOND, 7 * SECOND]] == [5 * SECOND, 6 * SECOND, 7 * SECOND]
    clock.now += 500
    assert target.now() == 7 * SECOND + 500
    assert target.restarts == 0


def test_a_restart_continues_after_the_newest_time_plus_the_time_since():
    clock = Clock()
    restarts = []
    target = TargetClock(clock, on_restart=restarts.append)
    target.map(10 * SECOND)
    clock.now += 2 * SECOND
    # the uptime started over
    assert target.map(100) == 12 * SECOND
    assert target.map(SECOND + 100) == 13 * SECOND
    assert target.restarts == 1
    assert target.offset == 12 * SECOND - 100
    assert restarts == [12 * SECOND]


def test_samples_a_little_out_of_order_are_no_restart():
    target = TargetClock(Clock())
    target.map(10 * SECOND)
    assert target.map(10 * SECOND - SECOND // 2) == 10 * SECOND - SECOND // 2
    assert target.restarts == 0


def test_after_a_possible_restart_any_earlier_time_is_one():
    clock = Clock()
    target = TargetClock(clock)
    target.map(SECOND // 2)
    target.restart()
    # later: the same run, connected again
    assert target.map(SECOND) == SECOND
    assert target.restarts == 0
    target.restart()
    clock.now += 10
    assert target.map(SECOND // 4) == SECOND + 10
    assert target.restarts == 1


def test_the_estimate_follows_the_newest_time_only():
    clock = Clock()
    target = TargetClock(clock)
    target.map(5 * SECOND)
    target.seen(4 * SECOND)
    assert target.now() == 5 * SECOND
    target.seen(6 * SECOND)
    assert target.now() == 6 * SECOND


# Timestamps of messages in the blip protocol


def test_the_timestamp_of_a_value_or_log_is_changed_in_place():
    value = Message.create_value("speed", 1.5, timestamp_ns=7).encode_cbor()
    assert cbor2.loads(retime(value, lambda t: t + 1_000_000_000_000))["Value"] == {
        "timestamp": 1_000_000_000_007, "path": "speed", "value": 1.5}
    log = Message.create_log(2, "hi", timestamp_ns=1_000_000, component="motor").encode_cbor()
    assert cbor2.loads(retime(log, lambda t: t - 1))["Log"]["timestamp"] == 999_999
    # unchanged, the same bytes
    assert retime(value, lambda t: t) is value


def test_a_timestamp_after_other_fields_is_changed_too():
    frame = cbor2.dumps({"Value": {"path": "speed", "value": 3, "timestamp": 10}})
    assert cbor2.loads(retime(frame, lambda t: t * 2)) == {"Value": {"path": "speed", "value": 3, "timestamp": 20}}


def test_no_timestamp_and_other_messages_stay_as_they_are():
    def fail(t: int) -> int:
        raise AssertionError("no time to map")

    log = Message.create_log(2, "now", timestamp_ns=0).encode_cbor()
    metadata = Message.create_metadata("speed", "rpm").encode_cbor()
    assert retime(log, fail) is log
    assert retime(metadata, fail) is metadata
