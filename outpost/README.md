# blip-outpost

Send the traces, log records and actions of your target to
[blip](https://stefanluethi.github.io/blip/), the telemetry viewer for embedded systems.

Your script talks to the target however it needs to: a serial port, a socket, a CAN bus,
a vendor library. It decodes what it receives and passes it on. The outpost serves blip
over a WebSocket, records the messages if asked, and calls your **actions** when blip
invokes them, e.g. a setpoint or a calibration.

The user guide is in the documentation:
[outpost](https://stefanluethi.github.io/blip/docs/usage/outpost.html). Its internals
are described in
[architecture/outpost](https://stefanluethi.github.io/blip/docs/architecture/outpost.html).

## Install

```bash
pip install "blip-outpost @ git+https://github.com/stefanluethi/blip#subdirectory=outpost"
```

The package is `blip-outpost`, imported as `blip_outpost`.

## Send to blip

```python
import random
import time

import blip_outpost as outpost

outpost.start()                                  # ws://127.0.0.1:3000/ws/outpost
outpost.send_metadata("room/temperature", "°C")  # units reach every blip, also later ones
while True:
    outpost.send_value("room/temperature", 21.0 + random.random())
    time.sleep(0.1)
```

`start` serves from a thread of its own and returns, so the script keeps its own loop.
Add the address in blip's **Connections** panel.

`send_value`, `send_metadata`, `send_log` and `send` may be called from any thread and
never block. A value is `None`, `int`, `float`, `bool`, `str`, a `list` of fixed length
or a `dict` with `str` keys, nested freely, timestamped now unless given.

## Actions

Functions decorated with `@action` are actions blip shows as buttons, fields, sliders
and dropdowns. Their parameters are taken from the type hints:

```python
from typing import Annotated, Literal

import blip_outpost as outpost
from blip_outpost import ActionError, Range, Unit


@outpost.action("motor/speed", readback="motor/speed_rpm")
def set_speed(rpm: Annotated[int, Range(0, 3000), Unit("rpm")] = 1500) -> None:
    """Speed setpoint"""
    if not link.is_open:
        raise ActionError("not connected")
    link.write(f"SPEED {rpm}\n".encode())


@outpost.action("log/level")
def set_log_level(level: Literal["debug", "info", "warn"] = "info") -> None:
    ...
```

Parameter types are `bool`, `int`, `float`, `str`, `Literal` of strings and `enum.Enum`
(offered by member name), optionally in `Annotated` with a `Range` and a `Unit`. A
default value is the initial value in blip.

An action succeeds when it returns and is rejected when it raises, `ActionError` with
the reason shown in blip. A plain function runs in a thread of its own, so it may block;
an `async` function runs in the event loop of the outpost. Invocations of one action are
carried out one after another, in the order blip sent them.

Methods become actions of the objects passed to `outpost.register(obj)`, so they share
the state of the object. Actions can be added, removed, enabled and disabled at any time
through `outpost.get_outpost().actions`, e.g. `set_enabled(False, reason="the link is
down")` while the link to the target is down; blip gets the new list right away and shows
the reason below the actions.

A script that only offers actions calls `outpost.wait()` after `start`, which blocks
until Ctrl+C.

## More

- `outpost.start(record="run.blip")` records everything sent, to open in blip later
  (**File > Open recording**).
- `Outpost(port=3001)` creates another outpost with the same methods, e.g. to serve
  several targets on different ports.
- In asyncio code, `async with Outpost() as outpost:` serves from the running event loop.
- `drain()` blocks until every blip has room for more messages, for scripts sending
  faster than blip takes them, e.g. a history.

## Command line

Targets that speak the blip protocol themselves, e.g. firmware with the C++ library
sonde, targets that send protobuf messages of an annotated schema, and recordings need no
script:

```bash
blip-outpost tcp 192.168.1.70 9000          # or --listen to wait for the target
blip-outpost udp 0.0.0.0 9000 --listen
blip-outpost protobuf ws://192.168.1.70/ws/robby --proto link.proto --uplink my.Uplink --downlink my.Downlink
blip-outpost replay recording.blip --speed 2
```

`protobuf` loads the schema at run time with `protoc` (no generated code) and connects to
the target's WebSocket: a binary frame holds one serialized message, a text frame JSON
values of the target. Annotations from `blip/options.proto` (it comes with the package)
say which trace, unit and scale each field becomes, and which oneof members of the
downlink message are actions:

```protobuf
import "blip/options.proto";

message Battery {
  option (blip.message) = {path: "battery"};
  uint64 t_ns = 1 [(blip.field).role = ROLE_TIMESTAMP];
  uint32 voltage_mv = 2 [(blip.field) = {name: "voltage", unit: "V", scale: 0.001}];
}
```

See the [user guide](https://stefanluethi.github.io/blip/docs/usage/outpost.html) for all
rules.

The target sends CBOR messages, several at once as a CBOR sequence (RFC 8742): a UDP
datagram holds complete messages, over TCP they may be split anywhere. Invocations of
blip are forwarded to the target. `--ws-port`, `--ws-host` and `--record` apply to every
command.

## Examples

The [`examples`](https://github.com/stefanluethi/blip/tree/main/outpost/examples)
folder of the repository holds scripts to try blip and to copy from. They are not part
of the package. In a checkout:

```bash
cd outpost
uv sync
uv run python examples/minimal.py
```

- `minimal.py`: a sine wave from a made-up sensor and an action, in plain blocking code.
- `simulator.py`: CPU load, simulated sensors at different rates, records, arrays,
  frames, log records and demo actions; written with asyncio and actions as methods.
- `fast.py`: a stress test of blip's ingest and rendering, by default about 100 000
  messages per second; `--history-s 600` first sends ten minutes of the past as fast as
  blip takes it.

## Develop

```bash
cd outpost
uv sync
uv run pytest
```

## Message schema (CBOR)

Messages sent in binary WebSocket frames match the Rust `proto` crate, see the
[wire protocol](https://stefanluethi.github.io/blip/docs/architecture/protocol.html):

- **Value**: `{ timestamp: int (ns), path: str, value: any }`, where `value` is a plain CBOR item: `None`, `int`, `float`, `bool`, `str`, a `list` of fixed length or a `dict` with `str` keys, nested freely
- **ValueMetadata**: `{ path: str, unit: str }`
- **LogRecord**: `{ timestamp: int (ns), level: int, component: str, message: str }`
- **Actions**: the list of all actions and their parameters
- **ActionResult**: `{ id: int, result: {"Ok": null} | {"Err": str} }`, the answer to an invocation
- **Request** (blip to the outpost): `{"Invoke": { id: int, path: str, args: dict }}`
