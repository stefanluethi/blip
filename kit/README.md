# blip-kit

The library [blip](https://github.com/stefanluethi/blip), the tracing tool for
embedded systems, is built from. Use it to build your own tracing UI with
[egui](https://github.com/emilk/egui): add the widgets you need in code, write your
own, read the data in the application itself, or keep it in a store of your own.

| Part | Feature | What it has |
|------|---------|-------------|
| `store` | always | The `Store` trait the widgets read through and the in-memory `MemoryStore`; the typed `Ingest` entry of a store; the `Source` trait for where the messages come from, which hand them to an `Inbox`; the `Session` that receives and sends each frame and holds the `Recorder`; recording files; the invoker of actions. No GUI. |
| `store::websocket` | `websocket` (default) | The WebSocket source, to connect to an outpost or a target. |
| `widgets` | `widgets` (default) | The time range with its controls and the timeline; the views: plot, states, table, XY plot, distribution, heatmap, log table and actions; the registry of the view types an application adds; the theme. |
| `workbench` | `workbench` | The layout of views the user arranges, the presets, the connections, the toasts and the file dialogs: what blip is made of. |

The wire types are re-exported as `blip_kit::proto`.

## A dashboard in code

```rust,ignore
use blip_kit::store::session::Session;
use blip_kit::widgets::time::TimeRange;
use blip_kit::widgets::view::{self, plot::Plot, ViewContext, Viewable as _};

let mut session = Session::recording();
session.add_source("target", my_source); // anything that implements `Source`: it hands bytes
                                          // in the blip protocol or typed values to an `Inbox`
let mut time = TimeRange::default();
let mut plot: Plot = view::showing(["motor/rpm"]);

// each frame
session.update(now);
time.update(&session.store, now);
plot.view(ui, &mut ViewContext::new(&session.store, &mut time, &mut session.invoker));
session.send_invocations();
```

[`examples/dashboard.rs`](examples/dashboard.rs) is a whole application: a simulated
target in the application itself, a plot, a table with a button in each row that
invokes an action, and the timeline.

```sh
cargo run --release -p blip-kit --example dashboard
```

The guides are in the
[documentation of blip](https://stefanluethi.github.io/blip/docs/library/index.html).

## Notes

- The library and the applications built on it compile natively and to WebAssembly.
- The widgets are drawn with egui: a release of egui that changes its API comes with
  a new major version of blip-kit.
- With `workbench`, the file dialogs come from `rfd`. On Linux, build it together with
  eframe's `wayland` feature, or install the development files of `libwayland`.
