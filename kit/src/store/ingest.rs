//! What puts data into a store: the [`Ingest`] trait and the inputs it takes.

use super::Level;
use super::time_base::TimeBase;
use super::value::{Rows, ValueRef};

/// What puts data into a store: takes the inputs of a model from the source named
/// `source`. [`MemoryStore`](super::MemoryStore) implements it; a session feeds any
/// store that does.
pub trait Ingest {
    /// Takes `input` from `source`, the name of its connection or file. The paths of the
    /// traces and logs are kept as they come, without the source.
    fn ingest(&mut self, source: &str, input: Input<'_>);

    /// Removes the samples and log records. Keeps what describes the sources (metadata,
    /// actions, clocks) and the latest values, with which a recording starts again.
    fn clear(&mut self);

    /// Drops everything, as a new store: a fresh start when the first source opens.
    fn reset(&mut self);

    /// What went wrong or needs telling since the last call, e.g. sources with different
    /// clocks. A store without notices keeps the default, which gives none.
    fn take_notices(&mut self) -> Vec<(Level, String)> {
        Vec::new()
    }
}

/// One input of the model, see [`Ingest::ingest`].
#[derive(Clone, Copy, Debug)]
pub enum Input<'a> {
    /// A value of the trace(s) at `path`.
    Sample {
        /// When the value was taken, in nanoseconds.
        timestamp: u64,
        /// The path of the trace, or of the record whose fields are traces.
        path: &'a str,
        /// The value.
        value: ValueRef<'a>,
    },
    /// Values of the trace(s) at `path`, taken at `times`. A block counts once for the
    /// readbacks of actions. `times` has a timestamp for each row of `values`.
    Samples {
        /// The path of the trace, or of the record whose fields are traces.
        path: &'a str,
        /// When each value was taken.
        times: Times<'a>,
        /// The values.
        values: Rows<'a>,
    },
    /// The newest value at `path` while samples are not kept (not recording): only the
    /// latest value of each path is kept, e.g. for the readbacks of actions. Counts once.
    Latest {
        /// When the value was taken, in nanoseconds.
        timestamp: u64,
        /// The path of the trace, or of the record whose fields are traces.
        path: &'a str,
        /// The value.
        value: ValueRef<'a>,
    },
    /// A log record.
    Log {
        /// When the record was made, in nanoseconds, 0 if it has no time.
        timestamp: u64,
        /// The path of the log, empty for the target as a whole.
        path: &'a str,
        /// The level, see `proto::level`.
        level: u8,
        /// The text of the record.
        message: &'a str,
    },
    /// What describes a trace: its unit, description, labels and expected range.
    Metadata(&'a proto::TraceMetadata),
    /// All actions of the source, replacing those it announced before.
    Actions(&'a [proto::ActionDescriptor]),
    /// The clock of the timestamps of the source, `None` if it says none.
    Clock(Option<TimeBase>),
}

/// When the values of a block were taken, in nanoseconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Times<'a> {
    /// Value `i` at `timestamp + i * period`.
    Period {
        /// When the first value was taken.
        timestamp: u64,
        /// The time between two values.
        period: u64,
    },
    /// The timestamp of each value.
    Each(&'a [u64]),
}

impl Times<'_> {
    /// The timestamp of value `index`, `None` past the list or past `u64::MAX`.
    pub fn timestamp(self, index: usize) -> Option<u64> {
        match self {
            Self::Period { timestamp, period } => {
                timestamp.checked_add(period.checked_mul(u64::try_from(index).ok()?)?)
            }
            Self::Each(list) => list.get(index).copied(),
        }
    }
}

impl<'a> From<&'a proto::SampleTimes> for Times<'a> {
    fn from(times: &'a proto::SampleTimes) -> Self {
        match times {
            proto::SampleTimes::Period { timestamp, period } => Self::Period {
                timestamp: *timestamp,
                period: *period,
            },
            proto::SampleTimes::Each(list) => Self::Each(list),
        }
    }
}
