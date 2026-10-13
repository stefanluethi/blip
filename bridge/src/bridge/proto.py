"""Protocol serialization and schema definition matching Rust proto crate."""

import time
from dataclasses import dataclass, asdict
from typing import Any, Dict, Optional, Union
import cbor2


@dataclass
class ValueMetadata:
    path: str
    unit: str

    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)


# A stream value as sent to blip: None (no value), int, float, bool, str, a list of
# fixed length or a dict with str keys, nested freely. It is encoded as a plain CBOR
# item, e.g. a dict becomes a CBOR map.
Data = Union[None, int, float, bool, str, list, dict]


@dataclass
class Value:
    timestamp: int  # Nanoseconds since Unix epoch
    path: str
    value: Data

    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)


@dataclass
class LogRecord:
    timestamp: int  # Nanoseconds in the producer's time domain
    level: int
    component: str
    message: str

    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)


@dataclass
class Message:
    """Protocol message wrapping Value, ValueMetadata, or LogRecord."""
    variant: str  # "Value", "ValueMetadata", or "Log"
    data: Union[Value, ValueMetadata, LogRecord]

    def to_dict(self) -> Dict[str, Any]:
        return {self.variant: self.data.to_dict()}

    def encode_cbor(self) -> bytes:
        """Encode message to CBOR binary format."""
        return cbor2.dumps(self.to_dict())

    @classmethod
    def create_value(cls, path: str, value: Data, timestamp_ns: Optional[int] = None) -> "Message":
        """Create a stream sample. Python ints encode as CBOR integers, so pass a float
        for a float stream (blip accepts integers in float streams anyway)."""
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
            )
        )


def decode_cbor_message(raw_bytes: bytes) -> Optional[Dict[str, Any]]:
    """Decode CBOR binary payload into dictionary."""
    try:
        data = cbor2.loads(raw_bytes)
        if isinstance(data, dict):
            return data
        return {"raw": data}
    except Exception:
        return None
