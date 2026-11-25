# blip-proto

The wire types of [blip](https://github.com/stefanluethi/blip), the tracing tool for
embedded systems: the messages a target sends (`Hello`, `Sample`, `Samples`,
`TraceMetadata`, `Log`, `Actions`, `ActionResult`) and the requests it takes
(`Invoke`). They are serde types, sent as CBOR.

```rust
use blip_proto::{Data, Message, Sample};

let message = Message::Sample(Sample {
    timestamp: 1_000_000, // nanoseconds
    path: "motor/rpm".to_owned(),
    value: Data::Float(1500.0),
});
```

`blip.cddl` describes the messages in CDDL (RFC 8610), for encoders in other
languages. The protocol is described in the
[protocol reference](https://stefanluethi.github.io/blip/docs/reference/protocol.html).
To show what a target sends, use [`blip-kit`](https://crates.io/crates/blip-kit), which
has the store and the widgets, or blip itself.
