# Outpost

> The user guide is in the documentation:
> [outpost](https://stefanluethi.github.io/blip/docs/usage/outpost.html)
> ([source](../doc/source/usage/outpost.rst)); its internals are described in
> [architecture/outpost](../doc/source/architecture/outpost.rst).

An asynchronous service next to a target, connecting it to blip over WebSocket. It serves
blip, records the messages and carries out the actions blip invokes. How it talks to
the target is a **protocol**, a Python class provided by the user of blip.

## Features

- **User-provided protocols**: a protocol is a small Python class against the public
  API (`from outpost import Protocol, action, ...`), loaded from a file or module.
- **Actions**: Python functions of the protocol that blip shows as buttons, fields,
  sliders and dropdowns, with their parameters taken from the type hints.
- **Simulator**: streams CPU load and simulated sensor measurements at different rates
  and synthetic log records, and offers demo actions. It is also the reference protocol.
- **Socket passthrough**: TCP and UDP for targets that speak the blip protocol
  themselves.
- **Recording and replay** of the messages sent to blip.
- **`uv` Integration**: Managed using `uv` and standard `pyproject.toml`.

## Quick Start

Sync dependencies using `uv`:

```bash
uv sync
```

Run the tests:

```bash
uv run pytest
```

### Running the Outpost

The simulator is the default protocol. It serves `ws://127.0.0.1:3000/ws/outpost`:

```bash
uv run outpost --protocol simulator --port 3000
```

The options of a protocol are set with `--option key=value` (`-o`), converted by the
type hints of its constructor. `--protocol-help` lists them:

```bash
uv run outpost --protocol simulator --protocol-help
uv run outpost --protocol simulator -o interval_ms=10 -o log_interval_ms=200
```

The protocols that come with the outpost:

- `simulator`: synthetic telemetry, logs and demo actions.
- `replay`: replays a recording (`-o path=recording.blip`, `-o speed=2` replays twice
  as fast). The actions of the recording are shown disabled.
- `tcp` and `udp`: pass the messages of a target that speaks the blip protocol
  through unchanged (`-o remote_host=… -o remote_port=… -o listen=true`), and
  forward invocations to it. The target may send several messages at once as a
  CBOR sequence (RFC 8742), their CBOR items one after another: a UDP datagram
  holds complete messages, over TCP they may be split anywhere. The outpost
  forwards each complete message, and drops data that is no CBOR.

Record the messages sent to blip with any protocol, to open them in blip later
(**File > Open recording**):

```bash
uv run outpost --protocol simulator --record simulator.blip
```

### Writing a Protocol

A protocol implements `run`, which the outpost runs while it serves and cancels when it
shuts down. It sends to blip with `send_value`, `send_metadata`, `send_log` or `send`.
Methods decorated with `@action` are its actions, sharing the link to the target:

```python
from typing import Annotated, Literal

from outpost import ActionError, LogLevel, Protocol, Range, Unit, action


class MyTarget(Protocol):
    def __init__(self, host: str, port: int = 9000) -> None:
        self.host, self.port = host, port
        self.link = None

    async def run(self) -> None:
        ...  # connect, translate what the target sends, e.g.
        await self.send_value("motor/speed_rpm", 1432)

    @action("motor/speed", readback="motor/speed_rpm")
    async def set_speed(self, rpm: Annotated[int, Range(0, 3000), Unit("rpm")]) -> None:
        """Speed setpoint"""
        if self.link is None:
            raise ActionError("not connected")
        ...

    @action("log/level")
    async def set_log_level(self, level: Literal["debug", "info", "warn"] = "info") -> None:
        ...
```

Run it by naming its file, followed by `:ClassName` if the file defines several
protocols, or its module:

```bash
uv run outpost --protocol ./my_target.py -o host=192.168.1.70
```

Or serve it from a script of your own with `outpost.run(MyTarget("192.168.1.70"))`.

Actions are carried out one after another per action, in the order blip sent them. An
action succeeds when it returns and is rejected when it raises, `ActionError` with the
reason shown in blip. Actions can be added, removed, enabled and disabled at runtime
through `self.actions`, e.g. `self.actions.set_enabled(False)` while the link to the
target is down; blip gets the new list right away.

Parameter types are `bool`, `int`, `float`, `str`, `Literal` of strings and
`enum.Enum` (offered by member name), optionally in `Annotated` with a `Range` and a
`Unit`. A default value is the initial value in blip.

## Protocol Data Schema (CBOR)

Messages sent in binary WebSocket frames match the Rust `proto` crate, see
`doc/source/architecture/protocol.rst`:

- **Value**: `{ timestamp: int (ns), path: str, value: any }`, where `value` is a plain CBOR item: `None`, `int`, `float`, `bool`, `str`, a `list` of fixed length or a `dict` with `str` keys, nested freely
- **ValueMetadata**: `{ path: str, unit: str }`
- **LogRecord**: `{ timestamp: int (ns), level: int, component: str, message: str }`
- **Actions**: the list of all actions and their parameters
- **ActionResult**: `{ id: int, result: {"Ok": null} | {"Err": str} }`, the answer to an invocation
- **Request** (blip to the outpost): `{"Invoke": { id: int, path: str, args: dict }}`
