"""Tests for the names of the messages of the protocol."""

import logging

import cbor2

from blip_outpost.proto import (
    METADATA_PREFIX,
    OLD_METADATA_PREFIX,
    OLD_SAMPLE_PREFIX,
    SAMPLE_PREFIX,
    Message,
    encode_value,
    metadata_path,
)
from blip_outpost.tracer import LogLevel, blip_level


def test_values_and_metadata_are_sent_as_sample_and_trace_metadata():
    sample = cbor2.dumps({"Sample": {"timestamp": 5, "path": "speed", "value": 1500}})
    assert Message.create_value("speed", 1500, 5).encode_cbor() == sample
    assert encode_value(5, "speed", 1500) == sample
    with_unit = Message.create_metadata("speed", "rpm").encode_cbor()
    without = Message.create_metadata("speed").encode_cbor()
    assert with_unit == cbor2.dumps({"TraceMetadata": {"path": "speed", "unit": "rpm"}})
    assert without == cbor2.dumps({"TraceMetadata": {"path": "speed"}})
    for frame in (sample, with_unit, without):
        assert b"Value" not in frame


def test_prefixes_tell_the_messages_apart():
    sample = Message.create_value("speed", 1500, 5).encode_cbor()
    old = cbor2.dumps({"Value": {"timestamp": 5, "path": "speed", "value": 1500}})
    assert sample.startswith(SAMPLE_PREFIX) and not sample.startswith(OLD_SAMPLE_PREFIX)
    assert old.startswith(OLD_SAMPLE_PREFIX) and not old.startswith(SAMPLE_PREFIX)
    meta = Message.create_metadata("speed", "rpm").encode_cbor()
    old_meta = cbor2.dumps({"ValueMetadata": {"path": "speed", "unit": "rpm"}})
    assert meta.startswith(METADATA_PREFIX) and not meta.startswith(OLD_METADATA_PREFIX)
    assert old_meta.startswith(OLD_METADATA_PREFIX) and not old_meta.startswith(METADATA_PREFIX)


def test_metadata_path():
    assert metadata_path(cbor2.dumps({"TraceMetadata": {"path": "speed", "unit": "rpm"}})) == "speed"
    assert metadata_path(cbor2.dumps({"ValueMetadata": {"path": "speed"}})) == "speed"
    assert metadata_path(Message.create_value("speed", 1, 5).encode_cbor()) is None
    assert metadata_path(b"") is None
    assert metadata_path(b"\xa1") is None
    assert metadata_path(cbor2.dumps({"TraceMetadata": {"path": 7}})) is None


def test_log_levels_are_those_of_the_protocol():
    assert [int(level) for level in LogLevel] == [0, 1, 2, 3, 4]
    assert [level.name for level in LogLevel] == ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"]
    assert blip_level(0) == LogLevel.TRACE
    assert blip_level(logging.CRITICAL) == LogLevel.ERROR
