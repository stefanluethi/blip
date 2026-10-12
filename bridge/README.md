# Protocol to WebSocket Bridge

A Python-based asynchronous bridge linking various data sources and protocols (TCP/UDP sockets or simulated hardware metrics) to a WebSocket server interface.

## Features

- **Protocol Agnostic Architecture**: Easily extendable base class for integrating new protocol handlers.
- **Simulator Mode**: Streams CPU load and simulated sensor measurements encoded via CBOR (`cbor2`), replicating the Rust simulator.
- **Socket Bridge Mode**: TCP and UDP packet forwarding over WebSockets.
- **CBOR Serialization**: Native support for binary `proto` messages matching the Rust `proto` library schema.
- **`uv` Integration**: Managed using `uv` and standard `pyproject.toml`.

## Quick Start

### Installation

Sync dependencies using `uv`:

```bash
uv sync
```

### Running the Bridge

#### Simulator Mode (Default)
Starts the WebSocket server on `ws://127.0.0.1:3000/ws/bridge` with simulated CPU and accelerometer telemetry:

```bash
uv run bridge --protocol simulator --port 3000
```

#### TCP Socket Mode
Bridges a TCP endpoint to WebSockets:

```bash
uv run bridge --protocol tcp --remote-host 127.0.0.1 --remote-port 9000
```

## Protocol Data Schema (CBOR)

Messages sent in binary WebSocket frames match the following Rust `proto` structure:

- **Value**: `{ timestamp: int (ns), path: str, value: float }`
- **ValueMetadata**: `{ path: str, unit: str }`
- **LogRecord**: `{ timestamp: int (ns), level: int, component: str, message: str }`
- **Message**: Tagged CBOR enum `Value`, `ValueMetadata`, or `Log`
- **Action**: Tagged CBOR enum for commands (e.g. `SetMotorSpeed(int)`)
