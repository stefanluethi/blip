//! Helpers of the tests of the store and the views: samples into [`Traces`], and typed
//! messages into a [`MemoryStore`], with the same inputs and answer as the byte entry of
//! the parser gives.

use crate::proto;

use super::ingest::Ingest;
use super::protocol::Parser;
use super::said::Said;
use super::traces::Traces;
use super::{Level, MemoryStore, said};

/// Appends a sample of `value` at `path` and `timestamp` to `traces`.
pub(crate) fn append(traces: &mut Traces, timestamp: u64, path: &str, value: proto::Data) {
    traces.append(&proto::Sample {
        timestamp,
        path: path.to_owned(),
        value,
    });
}

/// Hands `message`, received from `source`, to `store` as recording.
pub(crate) fn receive(
    store: &mut MemoryStore,
    source: &str,
    message: &proto::Message,
) -> Option<proto::ActionResult> {
    apply(store, source, message, true)
}

/// Hands `message` to `store` as while not recording: the samples are dropped.
pub(crate) fn receive_unrecorded(
    store: &mut MemoryStore,
    source: &str,
    message: &proto::Message,
) -> Option<proto::ActionResult> {
    apply(store, source, message, false)
}

fn apply(
    store: &mut MemoryStore,
    source: &str,
    message: &proto::Message,
    recording: bool,
) -> Option<proto::ActionResult> {
    let mut result = None;
    Parser::default().message(message, recording, |said| {
        match said {
            Said::ActionResult(answer) => result = Some(answer.clone()),
            Said::Unreadable { name, .. } if recording && name == Some("Samples") => {
                store.notify(Level::Error, said::malformed_block(source));
            }
            _ => {}
        }
        if let Some(input) = said.input() {
            store.ingest(source, input);
        }
    });
    result
}
