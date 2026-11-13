"""Tests for keeping the time of a target going forward when it restarts."""

import cbor2
import pytest

from blip_outpost.proto import Message, retime
from blip_outpost.timing import TargetClock

SECOND = 1_000_000_000
SHIFT = 1_000_000_000_000


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


def test_after_a_new_connection_a_late_sample_is_no_restart():
    clock = Clock()
    target = TargetClock(clock)
    target.map(5 * SECOND)
    target.restart()
    # buffered while the connection was lost
    assert target.map(5 * SECOND - 20_000_000) == 5 * SECOND - 20_000_000
    assert target.restarts == 0
    target.restart()
    clock.now += 10
    # more than 100 ms earlier
    assert target.map(4 * SECOND + 500_000_000) == 5 * SECOND + 10
    assert target.restarts == 1


def test_after_a_sure_restart_any_earlier_time_is_one():
    clock = Clock()
    target = TargetClock(clock)
    target.map(5 * SECOND)
    target.restart(sure=True)
    clock.now += 10
    assert target.map(5 * SECOND - 1_000) == 5 * SECOND + 10
    assert target.restarts == 1


def test_a_run_shorter_than_a_second_is_a_restart_too():
    clock = Clock()
    target = TargetClock(clock)
    # a crash loop: runs of half a second
    target.map(SECOND // 2)
    clock.now += 100_000_000
    assert target.map(50_000_000) == SECOND // 2 + 100_000_000
    assert target.restarts == 1
    # within a run, a sample a little out of order is none
    target.map(SECOND // 4)
    target.map(SECOND // 4 - 5_000_000)
    assert target.restarts == 1


def test_another_clock_is_shifted_like_the_target():
    clock = Clock()
    target = TargetClock(clock)
    target.map(10 * SECOND)
    target.map(100)
    assert target.shift(200) == 200 + target.offset
    # no restart, nor the newest time
    assert target.restarts == 1
    assert target.now() == 10 * SECOND + 1


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
    assert cbor2.loads(retime(value, lambda t: t + 1_000_000_000_000))["Sample"] == {
        "timestamp": 1_000_000_000_007, "path": "speed", "value": 1.5}
    log = Message.create_log(2, "hi", timestamp_ns=1_000_000, path="log/motor").encode_cbor()
    assert cbor2.loads(retime(log, lambda t: t - 1))["Log"]["timestamp"] == 999_999
    # unchanged, the same bytes
    assert retime(value, lambda t: t) is value


@pytest.mark.parametrize("variant", ["Sample", "Value"])
def test_a_timestamp_after_other_fields_is_changed_in_place_too(variant):
    value = {"a": [1.5, {"b": b"\x00\x01"}], "c": "x"}
    frame = cbor2.dumps({variant: {"path": "speed", "value": value, "timestamp": 10}})
    retimed = retime(frame, lambda t: t * 2)
    assert cbor2.loads(retimed) == {variant: {"path": "speed", "value": value, "timestamp": 20}}
    # the value stays as it was encoded, e.g. a float of 16 bits
    half = (b"\xa1" + cbor2.dumps(variant) + b"\xa3" + cbor2.dumps("path") + cbor2.dumps("p")
            + cbor2.dumps("value") + b"\xf9\x3e\x00" + cbor2.dumps("timestamp") + cbor2.dumps(3))
    assert retime(half, lambda t: t + 1) == half[:-1] + cbor2.dumps(4)


@pytest.mark.parametrize("variant", ["Sample", "Value"])
def test_a_timestamp_in_a_map_of_indefinite_length_is_found(variant):
    fields = b"".join(cbor2.dumps(item) for item in ["path", "p", "value", 1, "timestamp", 7])
    frame = b"\xa1" + cbor2.dumps(variant) + b"\xbf" + fields + b"\xff"
    assert cbor2.loads(retime(frame, lambda t: t + 1))[variant]["timestamp"] == 8


def test_no_timestamp_and_other_messages_stay_as_they_are():
    def fail(t: int) -> int:
        raise AssertionError("no time to map")

    log = Message.create_log(2, "now", timestamp_ns=0).encode_cbor()
    metadata = Message.create_metadata("speed", "rpm").encode_cbor()
    assert retime(log, fail) is log
    assert retime(metadata, fail) is metadata


def test_the_timestamp_of_a_block_with_a_period_is_changed_in_place():
    frame = Message.create_samples("adc", [1, 2, 3], timestamp_ns=7, period_ns=2).encode_cbor()
    assert cbor2.loads(retime(frame, lambda t: t + SHIFT)) == {
        "Samples": {"path": "adc", "values": [1, 2, 3], "timestamp": SHIFT + 7, "period": 2}}
    assert retime(frame, lambda t: t) is frame


def test_every_timestamp_of_a_block_is_changed_in_place():
    frame = Message.create_samples("adc", [1, 2, 3], timestamps_ns=[7, 9, 12]).encode_cbor()
    assert cbor2.loads(retime(frame, lambda t: t + SHIFT)) == {
        "Samples": {"path": "adc", "values": [1, 2, 3], "timestamps": [SHIFT + 7, SHIFT + 9, SHIFT + 12]}}
    indefinite = (b"\xa1" + cbor2.dumps("Samples") + b"\xa3" + cbor2.dumps("path") + cbor2.dumps("p")
                  + cbor2.dumps("timestamps") + b"\x9f" + cbor2.dumps(7) + cbor2.dumps(9) + b"\xff"
                  + cbor2.dumps("values") + b"\x9f\xf9\x3e\x00\xf9\x3e\x00\xff")
    retimed = retime(indefinite, lambda t: t + 1)
    block = cbor2.loads(retimed)["Samples"]
    assert block["timestamps"] == [8, 10]
    assert block["values"] == [1.5, 1.5]
    assert retimed.endswith(cbor2.dumps("values") + b"\x9f\xf9\x3e\x00\xf9\x3e\x00\xff")


def test_zero_and_malformed_timestamps_of_a_block_are_left_alone():
    def fail(t: int) -> int:
        raise AssertionError("no time to map")

    zero = Message.create_samples("adc", [1, 2], timestamps_ns=[0, 5]).encode_cbor()
    assert cbor2.loads(retime(zero, lambda t: t + 1))["Samples"]["timestamps"] == [0, 6]
    for bad in ([1.5], [-1]):
        frame = cbor2.dumps({"Samples": {"path": "p", "values": [1], "timestamps": bad}})
        assert retime(frame, fail) is frame
    none = Message.create_samples("adc", [1], timestamp_ns=0, period_ns=1).encode_cbor()
    assert retime(none, fail) is none
