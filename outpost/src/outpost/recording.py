"""Recording files of blip: a header and the CBOR messages as sent in WebSocket frames,
one after another as a CBOR sequence (RFC 8742), compressed with gzip.

blip opens these files, and saves the messages it received in the same format. Since
version 2, a text string between the messages names the connection of blip the messages
after it were received from. The outpost writes version 1, of one connection."""

import gzip
import io
import logging
import time
import zlib
from pathlib import Path
from typing import Iterator, Union

import cbor2

logger = logging.getLogger("outpost.recording")

FORMAT = "blip recording"
# The version written, of the messages of one connection.
VERSION = 1
# The newest version read.
READ_VERSION = 2

# The written messages are flushed this often, so a file is readable up to about
# this long before the outpost stopped, also if it was killed.
FLUSH_INTERVAL_SEC = 1.0


def header() -> bytes:
    """The first item of a recording file."""
    return cbor2.dumps({"format": FORMAT, "version": VERSION})


class RecordingWriter:
    """Writes the messages sent to blip to a recording file."""

    def __init__(self, path: Union[str, Path]) -> None:
        self.path = Path(path)
        self._file = gzip.open(self.path, "wb")
        self._file.write(header())
        self._last_flush = time.monotonic()
        self.messages = 0
        logger.info(f"Recording to {self.path}")

    def write(self, frame: bytes) -> None:
        """Appends the message of a binary WebSocket frame."""
        self._file.write(frame)
        self.messages += 1
        now = time.monotonic()
        if now - self._last_flush >= FLUSH_INTERVAL_SEC:
            self._file.flush()
            self._last_flush = now

    def close(self) -> None:
        """Finishes the file, does nothing if it is finished."""
        if self._file.closed:
            return
        self._file.close()
        logger.info(f"Recorded {self.messages} messages to {self.path}")


def _decompress(data: bytes, path: Union[str, Path]) -> bytes:
    """The contents of the gzip members in `data`, up to where it is cut off."""
    chunks = []
    while data:
        # 31 for a gzip header
        decompressor = zlib.decompressobj(wbits=31)
        try:
            chunks.append(decompressor.decompress(data))
        except zlib.error as e:
            logger.warning(f"{path} is damaged: {e}")
            break
        if not decompressor.eof:
            logger.warning(f"{path} is cut off")
            break
        data = decompressor.unused_data
    return b"".join(chunks)


def read_frames(path: Union[str, Path]) -> Iterator[bytes]:
    """The messages of a recording file as their CBOR items, byte for byte as recorded.
    Also reads uncompressed files. A file cut off in the middle of a message ends with
    the message before. The messages of all connections of a recording by blip are read
    one after another, without the names of their connections."""
    data = Path(path).read_bytes()
    if data.startswith(b"\x1f\x8b"):
        data = _decompress(data, path)

    stream = io.BytesIO(data)
    decoder = cbor2.CBORDecoder(stream)
    first = decoder.decode()
    if not isinstance(first, dict) or first.get("format") != FORMAT:
        raise ValueError(f"{path} is not a blip recording")
    if first.get("version", 0) > READ_VERSION:
        raise ValueError(
            f"{path} has version {first['version']}, the outpost reads up to {READ_VERSION}"
        )

    start = stream.tell()
    while start < len(data):
        try:
            item = decoder.decode()
        except (cbor2.CBORDecodeError, EOFError) as e:
            logger.warning(f"{path} ends with a message that cannot be read: {e}")
            return
        end = stream.tell()
        # messages are maps, a text string names the connection of the messages after it
        if not isinstance(item, str):
            yield data[start:end]
        start = end
