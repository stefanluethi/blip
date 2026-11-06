"""Protocol serialization and schema definition matching Rust proto crate."""

import enum
import io
import logging
import time
from dataclasses import dataclass
from typing import Any, Callable, Dict, List, Optional, Tuple, Union
import cbor2

logger = logging.getLogger(__name__)


class LogLevel(enum.IntEnum):
    """Levels of log records as understood by blip."""
    TRACE = 0
    DEBUG = 1
    INFO = 2
    WARN = 3
    ERROR = 4


@dataclass
class ValueMetadata:
    path: str
    unit: str

    def to_dict(self) -> Dict[str, Any]:
        return {"path": self.path, "unit": self.unit}


# A trace value as sent to blip: None (no value), int, float, bool, str, a list of
# fixed length or a dict with str keys, nested freely. It is encoded as a plain CBOR
# item, e.g. a dict becomes a CBOR map.
Data = Union[None, int, float, bool, str, list, dict]


@dataclass
class Value:
    timestamp: int  # Nanoseconds since Unix epoch
    path: str
    value: Data

    def to_dict(self) -> Dict[str, Any]:
        # not `asdict`, which copies the value, field by field: it takes twice as long
        return {"timestamp": self.timestamp, "path": self.path, "value": self.value}


@dataclass
class LogRecord:
    timestamp: int  # Nanoseconds in the producer's time domain
    level: int
    component: str
    message: str
    # Where the record comes from, e.g. "motor"; empty for the target as a whole
    path: str = ""

    def to_dict(self) -> Dict[str, Any]:
        record: Dict[str, Any] = {"timestamp": self.timestamp}
        # an empty path is left out, as blip defaults it
        if self.path:
            record["path"] = self.path
        record["level"] = self.level
        record["component"] = self.component
        record["message"] = self.message
        return record


@dataclass
class Message:
    """A message to blip: a Value, ValueMetadata, Log, Actions or ActionResult."""
    variant: str
    # a dataclass with to_dict or plain data
    data: Any

    def to_dict(self) -> Dict[str, Any]:
        data = self.data.to_dict() if hasattr(self.data, "to_dict") else self.data
        return {self.variant: data}

    def encode_cbor(self) -> bytes:
        """Encode message to CBOR binary format."""
        return cbor2.dumps(self.to_dict())

    @classmethod
    def create_value(cls, path: str, value: Data, timestamp_ns: Optional[int] = None) -> "Message":
        """Create a trace sample. Python ints encode as CBOR integers, so pass a float
        for a float trace (blip accepts integers in float traces anyway)."""
        if timestamp_ns is None:
            timestamp_ns = int(time.time() * 1e9)
        return cls(
            variant="Value",
            data=Value(timestamp=timestamp_ns, path=path, value=value)
        )

    @classmethod
    def create_metadata(cls, path: str, unit: str) -> "Message":
        return cls(
            variant="ValueMetadata",
            data=ValueMetadata(path=path, unit=unit)
        )

    @classmethod
    def create_log(
        cls,
        level: int,
        message: str,
        timestamp_ns: Optional[int] = None,
        component: str = "",
        path: str = "",
    ) -> "Message":
        if timestamp_ns is None:
            timestamp_ns = int(time.time() * 1e9)
        return cls(
            variant="Log",
            data=LogRecord(
                timestamp=timestamp_ns,
                level=level,
                component=component,
                message=message,
                path=path,
            )
        )

    @classmethod
    def create_actions(cls, descriptors: List[Dict[str, Any]]) -> "Message":
        """The list of all actions, see outpost.actions."""
        return cls(variant="Actions", data=descriptors)

    @classmethod
    def create_action_result(cls, id: int, error: Optional[str] = None) -> "Message":
        """The answer to the invocation `id`, rejected with `error` if given."""
        result = {"Ok": None} if error is None else {"Err": error}
        return cls(variant="ActionResult", data={"id": id, "result": result})


@dataclass
class Invocation:
    """A request of blip to carry out an action."""
    id: int
    path: str
    args: Dict[str, Any]


def variant_prefix(variant: str) -> bytes:
    """The start of the CBOR encoding of every message of `variant`: a map of one entry
    whose key is the name of the variant. Tells messages apart without decoding them."""
    return b"\xa1" + cbor2.dumps(variant)


ACTIONS_PREFIX = variant_prefix("Actions")
ACTION_RESULT_PREFIX = variant_prefix("ActionResult")
METADATA_PREFIX = variant_prefix("ValueMetadata")
VALUE_PREFIX = variant_prefix("Value")
LOG_PREFIX = variant_prefix("Log")
TIMESTAMP_KEY = cbor2.dumps("timestamp")

# The bytes after the first of a CBOR head, by its additional information
_ARGUMENT_BYTES = {**{info: 0 for info in range(24)}, 24: 1, 25: 2, 26: 4, 27: 8}


def _head(data: bytes, i: int) -> Optional[Tuple[int, int, int]]:
    """The major type and argument of the CBOR head at `i`, and where the head ends;
    `None` if there is none. An indefinite length has the argument -1."""
    if i >= len(data):
        return None
    major, info = data[i] >> 5, data[i] & 0x1F
    if info == 31 and major in (2, 3, 4, 5, 7):
        return major, -1, i + 1
    n = _ARGUMENT_BYTES.get(info)
    if n is None or i + 1 + n > len(data):
        return None
    if n == 0:
        return major, info, i + 1
    return major, int.from_bytes(data[i + 1:i + 1 + n], "big"), i + 1 + n


def _skip(data: bytes, i: int) -> Optional[int]:
    """Where the CBOR item at `i` ends, `None` if it is no complete item."""
    head = _head(data, i)
    if head is None:
        return None
    major, argument, i = head
    if major in (0, 1, 7):
        # a break (argument -1) ends an indefinite item, not an item of its own
        return None if argument == -1 else i
    if major in (2, 3):
        if argument >= 0:
            return i + argument if i + argument <= len(data) else None
        while i < len(data) and data[i] != 0xFF:
            i = _skip(data, i)
            if i is None:
                return None
        return i + 1 if i < len(data) else None
    if major == 6:
        return _skip(data, i)
    # arrays and maps
    items = argument * (2 if major == 5 else 1)
    if argument < 0:
        while i < len(data) and data[i] != 0xFF:
            i = _skip(data, i)
            if i is None:
                return None
        return i + 1 if i < len(data) else None
    for _ in range(items):
        i = _skip(data, i)
        if i is None:
            return None
    return i


def _timestamp_at(frame: bytes, i: int) -> Optional[Tuple[int, int, int]]:
    """The unsigned `timestamp` of the CBOR map at `i`, where it starts and ends;
    `None` if the map has none."""
    head = _head(frame, i)
    if head is None or head[0] != 5:
        return None
    _, entries, i = head
    count = 0
    while (entries < 0 and i < len(frame) and frame[i] != 0xFF) or count < entries:
        if frame.startswith(TIMESTAMP_KEY, i):
            at = i + len(TIMESTAMP_KEY)
            value = _head(frame, at)
            if value is None or value[0] != 0:
                return None
            return value[1], at, value[2]
        key_end = _skip(frame, i)
        i = _skip(frame, key_end) if key_end is not None else None
        if i is None:
            return None
        count += 1
    return None


def retime(frame: bytes, map_time: Callable[[int], int]) -> bytes:
    """The message in `frame` with the timestamp `t` of a Value or Log changed to
    `map_time(t)`, in place; other messages, and a timestamp 0 (none), as they are."""
    if frame.startswith(VALUE_PREFIX):
        start = len(VALUE_PREFIX)
    elif frame.startswith(LOG_PREFIX):
        start = len(LOG_PREFIX)
    else:
        return frame
    found = _timestamp_at(frame, start)
    if found is None or found[0] == 0:
        return frame
    t, at, end = found
    mapped = map_time(t)
    return frame if mapped == t else frame[:at] + cbor2.dumps(mapped) + frame[end:]


def split_items(data: bytes) -> Tuple[List[bytes], bytes]:
    """The complete CBOR items at the start of `data`, a CBOR sequence (RFC 8742) as
    targets send several messages at once, each encoded as it is, and the incomplete
    item after them. Raises ValueError if `data` holds something that is no CBOR."""
    stream = io.BytesIO(data)
    decoder = cbor2.CBORDecoder(stream)
    items = []
    start = 0
    while start < len(data):
        # a break outside of an item of indefinite length, which the decoder returns
        if data[start] == 0xFF:
            raise ValueError("unexpected break code")
        try:
            decoder.decode()
        except cbor2.CBORDecodeEOF:
            break
        except Exception as e:
            raise ValueError(f"not CBOR: {e}") from e
        end = stream.tell()
        items.append(data[start:end])
        start = end
    return items, data[start:]


def frames_of(data: bytes) -> List[bytes]:
    """The messages of `data`, complete ones one after another, a CBOR sequence; what is
    no CBOR or incomplete is dropped with an error."""
    try:
        frames, rest = split_items(data)
    except ValueError as e:
        logger.error(f"Dropping {len(data)} bytes that are no messages ({e}): {data[:32].hex()}")
        return []
    if rest:
        logger.error(f"Dropping {len(rest)} bytes of an incomplete message: {rest[:32].hex()}")
    return frames


def decode_request(raw_bytes: bytes) -> Invocation:
    """Decodes a request of blip, raises ValueError if it is not one."""
    try:
        data = cbor2.loads(raw_bytes)
    except Exception as e:
        raise ValueError(f"not CBOR: {e}") from e
    if not isinstance(data, dict) or set(data) != {"Invoke"}:
        raise ValueError("not an Invoke request")
    content = data["Invoke"]
    if not isinstance(content, dict):
        raise ValueError("malformed Invoke request")
    id, path, args = content.get("id"), content.get("path"), content.get("args")
    if not isinstance(id, int) or isinstance(id, bool) or not isinstance(path, str):
        raise ValueError("malformed Invoke request")
    return Invocation(id=id, path=path, args=args)


def decode_cbor_message(raw_bytes: bytes) -> Optional[Dict[str, Any]]:
    """Decode CBOR binary payload into dictionary."""
    try:
        data = cbor2.loads(raw_bytes)
        if isinstance(data, dict):
            return data
        return {"raw": data}
    except Exception:
        return None
