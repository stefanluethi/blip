use serde::{Deserialize, Serialize};

mod action;
mod data;

pub use action::{ActionDescriptor, ActionResult, Invocation, Param, ParamKind, Request};
pub use data::Data;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ValueMetadata {
    pub path: String,
    pub unit: String,
    // todo: enum values?
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Value {
    pub timestamp: u64,
    pub path: String,
    pub value: Data,
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
    pub level: u8,
    pub message: String,
}

/// A message from the target to blip.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Message {
    Value(Value),
    ValueMetadata(ValueMetadata),
    Log(LogRecord),
    /// All actions the target can carry out, replacing the ones announced before.
    Actions(Vec<ActionDescriptor>),
    ActionResult(ActionResult),
}
