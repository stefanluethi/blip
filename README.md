# blip

Lightweight web tracing tool for embedded systems.

blip shows live values as plots and log records as tables. It runs in the
browser (WASM) or as a native app, built with [egui](https://github.com/emilk/egui).
A small Python bridge connects an embedded target (or a simulator) to blip over
a WebSocket, using CBOR-encoded messages.

Try the web app at <https://stefanluethi.github.io/blip/> and read the
documentation at <https://stefanluethi.github.io/blip/docs/>.

```
embedded target  <-- TCP / UDP -->  bridge (Python)  <-- WebSocket / CBOR -->  blip (Rust)
```

| Directory | Contents |
|-----------|----------|
| `blip/`   | GUI app (Rust, egui), web and native |
| `proto/`  | Wire protocol types shared between blip and embedded targets |
| `bridge/` | Device-to-WebSocket bridge (Python) |
| `doc/`    | User documentation (Sphinx) |

## Prerequisites

- [Rust](https://rustup.rs) 1.85 or newer
- The WASM target and [Trunk](https://trunkrs.dev) for the web build:

  ```bash
  rustup target add wasm32-unknown-unknown
  cargo install --locked trunk
  ```

- [uv](https://docs.astral.sh/uv/) for the bridge and the documentation
- [Graphviz](https://graphviz.org) (`dot`) for building the documentation

## Build

### blip (native)

```bash
cargo build --release -p blip
```

The binary ends up in `target/release/blip`.

### blip (web)

```bash
cd blip
trunk build --release
```

The output goes to `blip/dist/`. Serve it with any static file server.

### bridge

```bash
cd bridge
uv sync
```

### Documentation

```bash
cd doc
make html     # output: doc/_build/html/index.html
```

See [`doc/README.md`](doc/README.md) for more options.

## Run

Start the bridge with the simulator (no hardware needed). It serves
`ws://127.0.0.1:3000/ws/bridge`:

```bash
cd bridge
uv run bridge --protocol simulator --port 3000
```

Then start blip, either in the browser:

```bash
cd blip
trunk serve   # open http://127.0.0.1:8080
```

or natively:

```bash
cargo run --release -p blip
```

To bridge a real TCP endpoint instead of the simulator:

```bash
uv run bridge --protocol tcp --remote-host 127.0.0.1 --remote-port 9000
```

Run `uv run bridge --help` for all options.

## Checks

`check.sh` runs the CI-style checks: `cargo check` for native and WASM,
`rustfmt`, `clippy`, the tests and a `trunk build`:

```bash
./check.sh
```

## License

MIT, see [LICENSE](LICENSE).
