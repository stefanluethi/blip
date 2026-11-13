use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

mod action;
mod data;
mod hello;
mod samples;

pub use action::{ActionDescriptor, ActionResult, Invocation, Param, ParamKind, Request};
pub use data::Data;
pub use hello::{Clock, Hello};
pub use samples::{SampleTimes, Samples};

/// The version of the protocol of this crate, sent in [`Hello::version`].
pub const VERSION: u32 = 1;

/// The levels of [`LogRecord::level`]; blip shows a level above `ERROR` as an error.
pub mod level {
    pub const TRACE: u8 = 0;
    pub const DEBUG: u8 = 1;
    pub const INFO: u8 = 2;
    pub const WARN: u8 = 3;
    pub const ERROR: u8 = 4;
}

/// A single sample of a trace.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Sample {
    pub timestamp: u64,
    pub path: String,
    pub value: Data,
}

/// The range a trace's values are expected in.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct ExpectedRange {
    pub min: f64,
    pub max: f64,
}

/// What a target says about a trace. A newer one of the same path replaces the older.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct TraceMetadata {
    pub path: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub unit: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The names of the values of an integer trace.
    #[serde(rename = "enum", default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<i64, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<ExpectedRange>,
}

/// A log record. Records of older targets may carry a `component`, which is ignored.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LogRecord {
    pub timestamp: u64,
    /// Where the record comes from, levels separated by "/" as in the paths of values,
    /// e.g. "motor" or "robot/arm". Empty for the target as a whole, also in recordings
    /// made before logs had a path. Left out on the wire when empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    /// One of [`level`].
    pub level: u8,
    pub message: String,
}

/// A message from the target to blip.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Message {
    #[serde(alias = "Value")]
    Sample(Sample),
    #[serde(alias = "ValueMetadata")]
    TraceMetadata(TraceMetadata),
    Log(LogRecord),
    /// All actions the target can carry out, replacing the ones announced before.
    Actions(Vec<ActionDescriptor>),
    ActionResult(ActionResult),
    /// What the target and its connection are, first on a connection.
    Hello(Hello),
    /// A block of samples of one trace.
    Samples(Samples),
}
