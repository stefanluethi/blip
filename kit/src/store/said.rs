//! What the parser makes of a message: the inputs of the model and what only the
//! session handles (answers to invocations, messages that cannot be read).

use super::ingest::{Input, Times};
use super::time_base::TimeBase;
use super::value::{Rows, ValueRef};

/// A message as the session sees it, borrowed from the parser's buffers or the bytes of
/// the message.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Said<'a> {
    Hello(&'a proto::Hello),
    Metadata(&'a proto::TraceMetadata),
    Actions(&'a [proto::ActionDescriptor]),
    ActionResult(&'a proto::ActionResult),
    Sample {
        timestamp: u64,
        path: &'a str,
        value: ValueRef<'a>,
    },
    Samples {
        path: &'a str,
        times: Times<'a>,
        values: Rows<'a>,
    },
    Latest {
        timestamp: u64,
        path: &'a str,
        value: ValueRef<'a>,
    },
    Log {
        timestamp: u64,
        path: &'a str,
        level: u8,
        message: &'a str,
    },
    /// A message that cannot be read, only from bytes: its name if it has one, why, and
    /// its bytes as they arrived.
    Unreadable {
        name: Option<&'a str>,
        error: &'a str,
        item: &'a [u8],
    },
}

impl<'a> Said<'a> {
    /// A sample, or with `full` false (not recording) the latest value at `path`.
    pub(crate) fn sample(full: bool, timestamp: u64, path: &'a str, value: ValueRef<'a>) -> Self {
        if full {
            Said::Sample {
                timestamp,
                path,
                value,
            }
        } else {
            Said::Latest {
                timestamp,
                path,
                value,
            }
        }
    }

    /// The input of the store for this message, `None` for the answers to invocations
    /// and the messages that cannot be read.
    pub(crate) fn input(&self) -> Option<Input<'a>> {
        Some(match *self {
            Said::Hello(hello) => Input::Clock(hello.clock.map(TimeBase::from)),
            Said::Metadata(metadata) => Input::Metadata(metadata),
            Said::Actions(actions) => Input::Actions(actions),
            Said::Sample {
                timestamp,
                path,
                value,
            } => Input::Sample {
                timestamp,
                path,
                value,
            },
            Said::Samples {
                path,
                times,
                values,
            } => Input::Samples {
                path,
                times,
                values,
            },
            Said::Latest {
                timestamp,
                path,
                value,
            } => Input::Latest {
                timestamp,
                path,
                value,
            },
            Said::Log {
                timestamp,
                path,
                level,
                message,
            } => Input::Log {
                timestamp,
                path,
                level,
                message,
            },
            Said::ActionResult(_) | Said::Unreadable { .. } => return None,
        })
    }
}

/// The notice for a block of samples of `source` that was dropped as malformed.
pub(crate) fn malformed_block(source: &str) -> String {
    if source.is_empty() {
        "Dropped a malformed block of samples".to_owned()
    } else {
        format!("Dropped a malformed block of samples from {source}")
    }
}
