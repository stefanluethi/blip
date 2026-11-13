"""Protocol serialization and schema definition matching Rust proto crate."""

import dataclasses
import enum
import io
import logging
import time
from dataclasses import dataclass
from typing import Any, Callable, ClassVar, Dict, List, Mapping, Optional, Protocol, Sequence, Tuple, Union
import cbor2

logger = logging.getLogger(__name__)


CLOCKS = ("epoch", "boot")
PROTOCOL_VERSION = 1


class LogLevel(enum.IntEnum):
    """Levels of log records as understood by blip."""
    TRACE = 0
    DEBUG = 1
    INFO = 2
    WARN = 3
    ERROR = 4


@dataclass
class TraceMetadata:
    path: str
    unit: str = ""
    description: str = ""
    # names of integer values, the wire key is "enum"
    labels: Optional[Mapping[int, str]] = None
    # the expected (min, max) of the values, the wire key is "range"
    value_range: Optional[Tuple[float, float]] = None

    def __post_init__(self) -> None:
        if self.labels is not None:
            for key, label in self.labels.items():
                if isinstance(key, bool) or not isinstance(key, int) or not isinstance(label, str):
                    raise ValueError(f"labels map integers to text, not {key!r}: {label!r}")
            self.labels = dict(self.labels) or None
        if self.value_range is not None:
            try:
                low, high = self.value_range
            except (TypeError, ValueError):
                raise ValueError(f"value_range is (min, max), not {self.value_range!r}") from None
            for bound in (low, high):
                if isinstance(bound, bool) or not isinstance(bound, (int, float)) or bound != bound:
                    raise ValueError(f"value_range is (min, max) of numbers, not {self.value_range!r}")
            if low > high:
                raise ValueError(f"value_range has min above max: {self.value_range!r}")
            self.value_range = (low, high)

    def to_dict(self) -> Dict[str, Any]:
        data: Dict[str, Any] = {"path": self.path}
        if self.unit:
            data["unit"] = self.unit
        if self.description:
            data["description"] = self.description
        if self.labels:
            data["enum"] = dict(self.labels)
        if self.value_range is not None:
            data["range"] = {"min": self.value_range[0], "max": self.value_range[1]}
        return data


class DataclassInstance(Protocol):
    """An instance of a dataclass, a record of its fields."""
    __dataclass_fields__: ClassVar[Dict[str, Any]]


class SupportsToList(Protocol):
    """E.g. a numpy array or scalar, sent as what `tolist` returns."""
    def tolist(self) -> Any: ...


# A trace value as sent to blip: None (no value), int, float, bool, str, a list of
# fixed length or a dict with str keys, nested freely. It is encoded as a plain CBOR
# item, e.g. a dict becomes a CBOR map. A dataclass is a record like a dict of its
# fields, and what has a `tolist` method, e.g. a numpy array or scalar, is sent as
# what it returns.
Data = Union[None, int, float, bool, str, list, dict, DataclassInstance, SupportsToList]


def encode_value(timestamp_ns: int, path: str, value: Data) -> bytes:
    """The bytes of a Sample message, as `Message.create_value(...).encode_cbor()`."""
    return cbor2.dumps({"Sample": {"timestamp": timestamp_ns, "path": path, "value": value}}, default=encode_default)


def encode_default(encoder: Any, value: Any) -> None:
    """Encodes what cbor2 does not know itself: a dataclass as a map of its fields, and
    e.g. numpy arrays and scalars as what their `tolist` returns."""
    if dataclasses.is_dataclass(value) and not isinstance(value, type):
        # not `asdict`, which copies nested values; cbor2 comes back for them
        encoder.encode({f.name: getattr(value, f.name) for f in dataclasses.fields(value)})
        return
    tolist = getattr(value, "tolist", None)
    if callable(tolist):
        encoder.encode(tolist())
        return
    raise cbor2.CBOREncodeTypeError(f"cannot send a value of type {type(value).__name__}")


@dataclass
class Hello:
    """The first message of a target or an outpost: who sends and which clock the
    timestamps are in."""
    name: str = ""
    # "epoch" (since the Unix epoch) or "boot" (since the start of the target), "" if unknown
    clock: str = ""
    sender: str = ""
    version: int = PROTOCOL_VERSION

    def __post_init__(self) -> None:
        if self.clock and self.clock not in CLOCKS:
            raise ValueError(f"clock is epoch or boot, not {self.clock!r}")

    def to_dict(self) -> Dict[str, Any]:
        data: Dict[str, Any] = {"version": self.version}
        if self.name:
            data["name"] = self.name
        if self.clock:
            data["clock"] = self.clock
        if self.sender:
            data["sender"] = self.sender
        return data


@dataclass
class Sample:
    timestamp: int  # Nanoseconds since Unix epoch
    path: str
    value: Data

    def to_dict(self) -> Dict[str, Any]:
        # not `asdict`, which copies the value, field by field: it takes twice as long
        return {"timestamp": self.timestamp, "path": self.path, "value": self.value}


def _is_int(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


@dataclass
class Samples:
    """A block of samples of one trace: sample `i` at `timestamp + i * period`, or at
    `timestamps[i]`, in nanoseconds."""
    path: str
    values: List[Data]
    timestamp: Optional[int] = None
    period: Optional[int] = None
    timestamps: Optional[List[int]] = None

    def __post_init__(self) -> None:
        if self.timestamps is not None:
            if self.timestamp is not None or self.period is not None:
                raise ValueError("either timestamps, or timestamp and period")
            self.timestamps = list(self.timestamps)
            times = self.timestamps
            if len(times) != len(self.values):
                raise ValueError(f"{len(self.values)} values but {len(times)} timestamps")
        else:
            if self.timestamp is None or self.period is None:
                raise ValueError("either timestamps, or timestamp and period")
            times = [self.timestamp, self.period]
        for t in times:
            if not _is_int(t):
                raise ValueError(f"timestamps and periods are integers, not {t!r}")
            if t < 0:
                raise ValueError(f"a negative timestamp or period: {t}")

    def to_dict(self) -> Dict[str, Any]:
        data: Dict[str, Any] = {"path": self.path, "values": self.values}
        if self.timestamps is not None:
            data["timestamps"] = self.timestamps
        else:
            data["timestamp"] = self.timestamp
            data["period"] = self.period
        return data


@dataclass
class LogRecord:
    timestamp: int  # Nanoseconds in the producer's time domain
    level: int
    message: str
    # Where the record comes from, e.g. "motor"; empty for the target as a whole
    path: str = ""

    def to_dict(self) -> Dict[str, Any]:
        record: Dict[str, Any] = {"timestamp": self.timestamp}
        # an empty path is left out, as blip defaults it
        if self.path:
            record["path"] = self.path
        record["level"] = self.level
        record["message"] = self.message
        return record


@dataclass
class Message:
    """A message to blip: a Sample, TraceMetadata, Log, Actions or ActionResult."""
    variant: str
    # a dataclass with to_dict or plain data
    data: Any

    def to_dict(self) -> Dict[str, Any]:
        data = self.data.to_dict() if hasattr(self.data, "to_dict") else self.data
        return {self.variant: data}

    def encode_cbor(self) -> bytes:
        """Encode message to CBOR binary format."""
        return cbor2.dumps(self.to_dict(), default=encode_default)

    @classmethod
    def create_value(cls, path: str, value: Data, timestamp_ns: Optional[int] = None) -> "Message":
        """Create a trace sample. Python ints encode as CBOR integers, so pass a float
        for a float trace (blip accepts integers in float traces anyway)."""
        if timestamp_ns is None:
            timestamp_ns = int(time.time() * 1e9)
        return cls(
            variant="Sample",
            data=Sample(timestamp=timestamp_ns, path=path, value=value)
        )

    @classmethod
    def create_metadata(
        cls,
        path: str,
        unit: str = "",
        *,
        description: str = "",
        labels: Optional[Mapping[int, str]] = None,
        value_range: Optional[Tuple[float, float]] = None,
    ) -> "Message":
        return cls(
            variant="TraceMetadata",
            data=TraceMetadata(path=path, unit=unit, description=description, labels=labels, value_range=value_range)
        )

    @classmethod
    def create_hello(cls, name: str = "", clock: str = "", sender: str = "") -> "Message":
        """The Hello of a sender: its `name`, the `clock` of its timestamps ("epoch" or
        "boot") and who `sender` is, e.g. ``blip-outpost 0.1.0``."""
        return cls(variant="Hello", data=Hello(name=name, clock=clock, sender=sender))

    @classmethod
    def create_samples(
        cls,
        path: str,
        values: Sequence[Data],
        *,
        timestamp_ns: Optional[int] = None,
        period_ns: Optional[int] = None,
        timestamps_ns: Optional[Sequence[int]] = None,
    ) -> "Message":
        """Create a block of samples of one trace: the first at `timestamp_ns` and then
        one every `period_ns`, or one at each of `timestamps_ns`. Raises `ValueError`
        for anything else, for values and timestamps of different numbers and for
        negative times."""
        return cls(
            variant="Samples",
            data=Samples(
                path=path,
                values=list(values),
                timestamp=timestamp_ns,
                period=period_ns,
                timestamps=None if timestamps_ns is None else list(timestamps_ns),
            ),
        )

    @classmethod
    def create_log(
        cls,
        level: int,
        message: str,
        timestamp_ns: Optional[int] = None,
        path: str = "",
    ) -> "Message":
        if timestamp_ns is None:
            timestamp_ns = int(time.time() * 1e9)
        return cls(
            variant="Log",
            data=LogRecord(
                timestamp=timestamp_ns,
                level=level,
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
SAMPLE_PREFIX = variant_prefix("Sample")
# The names of protocol versions before 3: read and forwarded, never written.
OLD_SAMPLE_PREFIX = variant_prefix("Value")
SAMPLES_PREFIX = variant_prefix("Samples")
METADATA_PREFIX = variant_prefix("TraceMetadata")
OLD_METADATA_PREFIX = variant_prefix("ValueMetadata")
HELLO_PREFIX = variant_prefix("Hello")
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


def _key_value_at(frame: bytes, i: int, key: bytes) -> Optional[int]:
    """Where the value of the first top-level `key`, a CBOR-encoded text, of the CBOR map
    at `i` starts; `None` if the map has none."""
    head = _head(frame, i)
    if head is None or head[0] != 5:
        return None
    _, entries, i = head
    count = 0
    while (entries < 0 and i < len(frame) and frame[i] != 0xFF) or count < entries:
        if frame.startswith(key, i):
            return i + len(key)
        key_end = _skip(frame, i)
        i = _skip(frame, key_end) if key_end is not None else None
        if i is None:
            return None
        count += 1
    return None


def _timestamp_at(frame: bytes, i: int) -> Optional[Tuple[int, int, int]]:
    """The unsigned `timestamp` of the CBOR map at `i`, where it starts and ends;
    `None` if the map has none."""
    at = _key_value_at(frame, i, TIMESTAMP_KEY)
    if at is None:
        return None
    value = _head(frame, at)
    if value is None or value[0] != 0:
        return None
    return value[1], at, value[2]


def metadata_path(frame: bytes) -> Optional[str]:
    """The path of a TraceMetadata (or old ValueMetadata) message, None for anything
    else or if it does not decode or has no text path."""
    if not frame.startswith((METADATA_PREFIX, OLD_METADATA_PREFIX)):
        return None
    try:
        path = next(iter(cbor2.loads(frame).values()))["path"]
    except Exception:
        return None
    return path if isinstance(path, str) else None


def _retime_timestamp(frame: bytes, start: int, map_time: Callable[[int], int]) -> bytes:
    found = _timestamp_at(frame, start)
    if found is None or found[0] == 0:
        return frame
    t, at, end = found
    mapped = map_time(t)
    return frame if mapped == t else frame[:at] + cbor2.dumps(mapped) + frame[end:]


def _retime_timestamps(frame: bytes, start: int, map_time: Callable[[int], int]) -> bytes:
    """`frame` with each entry of the `timestamps` list of the block at `start` mapped;
    `frame` itself if nothing changes or an entry is no unsigned integer."""
    at = _key_value_at(frame, start, cbor2.dumps("timestamps"))
    head = _head(frame, at) if at is not None else None
    if head is None or head[0] != 4:
        return frame
    _, count, i = head
    first = i
    entries = []
    changed = False
    while (count < 0 and i < len(frame) and frame[i] != 0xFF) or len(entries) < count:
        item = _head(frame, i)
        if item is None or item[0] != 0:
            return frame
        t, end = item[1], item[2]
        mapped = map_time(t) if t != 0 else t
        changed = changed or mapped != t
        entries.append(cbor2.dumps(mapped) if mapped != t else frame[i:end])
        i = end
    if not changed or (count < 0 and i >= len(frame)):
        return frame
    return frame[:first] + b"".join(entries) + frame[i:]


def retime(frame: bytes, map_time: Callable[[int], int]) -> bytes:
    """The message in `frame` with the timestamp `t` of a Sample (or Value) or Log changed to
    `map_time(t)`, in place; a Samples block has its one `timestamp` or each entry of
    `timestamps` changed. Other messages, and a timestamp 0 (none), as they are."""
    for prefix in (SAMPLE_PREFIX, OLD_SAMPLE_PREFIX, LOG_PREFIX):
        if frame.startswith(prefix):
            return _retime_timestamp(frame, len(prefix), map_time)
    if frame.startswith(SAMPLES_PREFIX):
        start = len(SAMPLES_PREFIX)
        if _key_value_at(frame, start, TIMESTAMP_KEY) is not None:
            return _retime_timestamp(frame, start, map_time)
        return _retime_timestamps(frame, start, map_time)
    return frame


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
